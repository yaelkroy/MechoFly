using System;
using System.Collections.Generic;

namespace MechoFly
{
    internal sealed class BoundedReplayStore
    {
        public const int MaximumFrames = 240;
        public const int MaximumComparisonFrames = 120;

        private readonly NeuralFrame[] _frames;
        private int _next;
        private int _count;

        public BoundedReplayStore()
        {
            _frames = new NeuralFrame[MaximumFrames];
        }

        public int Count { get { return _count; } }

        public void Add(NeuralState state, NeuralSummary summary)
        {
            if (state == null || summary == null)
            {
                throw new ArgumentNullException("state");
            }
            _frames[_next] = new NeuralFrame(state.CloneDeep(), summary.CloneValue());
            _next = (_next + 1) % _frames.Length;
            if (_count < _frames.Length) _count++;
        }

        public ReplayWindow SnapshotRecent(int requestedFrames)
        {
            if (requestedFrames < 2 || requestedFrames > MaximumComparisonFrames)
            {
                throw new ArgumentOutOfRangeException("requestedFrames");
            }
            if (_count < 2)
            {
                throw new InvalidOperationException("Replay does not contain two frames yet.");
            }
            int take = Math.Min(requestedFrames, _count);
            NeuralFrame[] result = new NeuralFrame[take];
            int start = (_next - take + _frames.Length) % _frames.Length;
            int i;
            for (i = 0; i < take; i++)
            {
                int index = (start + i) % _frames.Length;
                result[i] = _frames[index].CloneDeep();
            }
            return new ReplayWindow(result);
        }
    }

    internal sealed class ReplayWindow
    {
        public readonly NeuralFrame[] Frames;

        public ReplayWindow(NeuralFrame[] frames)
        {
            if (frames == null || frames.Length < 2 ||
                frames.Length > BoundedReplayStore.MaximumComparisonFrames)
            {
                throw new ArgumentException("Replay window is outside its bounded contract.", "frames");
            }
            Frames = frames;
        }
    }

    internal sealed class ComparisonFrame
    {
        public readonly NeuralFrame Actual;
        public readonly NeuralFrame Alternative;
        public readonly int OffsetMilliseconds;

        public ComparisonFrame(NeuralFrame actual, NeuralFrame alternative, int offsetMilliseconds)
        {
            Actual = actual;
            Alternative = alternative;
            OffsetMilliseconds = offsetMilliseconds;
        }
    }

    internal sealed class ComparisonSequence
    {
        public readonly ComparisonFrame[] Frames;
        public readonly StimulationReceipt Receipt;

        public ComparisonSequence(ComparisonFrame[] frames, StimulationReceipt receipt)
        {
            Frames = frames;
            Receipt = receipt;
        }
    }

    internal static class ComparisonBuilder
    {
        public static ComparisonFrame[] BuildDetached(
            NeuralEngine engine,
            ReplayWindow window,
            StimulationPlan plan)
        {
            if (engine == null || window == null || plan == null)
            {
                throw new ArgumentNullException("engine");
            }
            plan.Validate(engine.NeuronCount);
            NeuralState alternativeState = window.Frames[0].State.CloneDeep();
            ComparisonFrame[] result = new ComparisonFrame[window.Frames.Length];
            result[0] = new ComparisonFrame(
                window.Frames[0].CloneDeep(),
                new NeuralFrame(alternativeState.CloneDeep(), window.Frames[0].Summary.CloneValue()),
                0);

            int i;
            for (i = 1; i < window.Frames.Length; i++)
            {
                ExternalDrive drive = plan.DriveAtFrame(i);
                NeuralSummary alternativeSummary = engine.Step(alternativeState, drive);
                result[i] = new ComparisonFrame(
                    window.Frames[i].CloneDeep(),
                    new NeuralFrame(alternativeState.CloneDeep(), alternativeSummary.CloneValue()),
                    checked(i * NeuralEngine.StepMilliseconds));
            }
            return result;
        }

        public static bool HasAnyDifference(ComparisonFrame[] frames)
        {
            int i;
            for (i = 1; i < frames.Length; i++)
            {
                if (!string.Equals(
                    frames[i].Actual.State.Digest(),
                    frames[i].Alternative.State.Digest(),
                    StringComparison.Ordinal))
                {
                    return true;
                }
            }
            return false;
        }
    }
}

