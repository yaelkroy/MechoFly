using System;
using System.Threading;

namespace MechoFly
{
    internal sealed class SimulationCoordinator : IDisposable
    {
        private readonly object _gate;
        private readonly NeuralEngine _engine;
        private readonly BoundedReplayStore _replay;
        private readonly NeuralState _liveState;
        private Thread _thread;
        private volatile bool _stopping;
        private volatile bool _paused;
        private NeuralSummary _latestSummary;

        public SimulationCoordinator(bool startBackground)
        {
            _gate = new object();
            _engine = NeuralEngine.CreateSyntheticDemo(1536, 12, 0x4d6563686f466c79UL);
            _replay = new BoundedReplayStore();
            _liveState = _engine.CreateState();
            _latestSummary = new NeuralSummary(NeuralEngine.GroupCount);
            if (startBackground)
            {
                _thread = new Thread(RunLoop);
                _thread.IsBackground = true;
                _thread.Name = "MechoFly modeled neural loop";
                _thread.Start();
            }
        }

        public NeuralEngine Engine { get { return _engine; } }
        public bool Paused { get { return _paused; } }

        public void SetPaused(bool paused)
        {
            _paused = paused;
        }

        public void StepForTest()
        {
            lock (_gate)
            {
                StepLocked();
            }
        }

        public NeuralFrame GetLatestFrame()
        {
            lock (_gate)
            {
                return new NeuralFrame(_liveState.CloneDeep(), _latestSummary.CloneValue());
            }
        }

        public int GetReplayCount()
        {
            lock (_gate)
            {
                return _replay.Count;
            }
        }

        public ComparisonSequence BuildPreview(StimulationPlan plan, int requestedFrames)
        {
            lock (_gate)
            {
                plan.Validate(_engine.NeuronCount);
                string before = _liveState.Digest();
                ReplayWindow window = _replay.SnapshotRecent(requestedFrames);
                ComparisonFrame[] frames = ComparisonBuilder.BuildDetached(_engine, window, plan);
                string after = _liveState.Digest();
                bool unchanged = string.Equals(before, after, StringComparison.Ordinal);
                StimulationReceipt receipt = new StimulationReceipt();
                receipt.Status = unchanged ? "PASS" : "FAIL";
                receipt.PolicyVersion = StimulationPlan.PolicyVersion;
                receipt.PlanId = plan.PlanId;
                receipt.PlanDigest = plan.Digest();
                receipt.Source = plan.Source;
                receipt.AuthoredBy = plan.AuthoredBy;
                receipt.GeneratedUtc = DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ");
                receipt.FrameCount = frames.Length;
                receipt.TargetCount = plan.Targets.Length;
                receipt.Amplitude = plan.Amplitude;
                receipt.DurationMilliseconds = plan.DurationMilliseconds;
                receipt.LiveStateBefore = before;
                receipt.LiveStateAfter = after;
                receipt.LiveStateUnchanged = unchanged;
                receipt.PreviewOnly = plan.PreviewOnly;
                receipt.HardwareSideEffects = false;
                if (!unchanged)
                {
                    throw new InvalidOperationException("Preview generation changed live state.");
                }
                return new ComparisonSequence(frames, receipt);
            }
        }

        private void RunLoop()
        {
            while (!_stopping)
            {
                DateTime started = DateTime.UtcNow;
                if (!_paused)
                {
                    lock (_gate)
                    {
                        StepLocked();
                    }
                }
                int elapsed = (int)(DateTime.UtcNow - started).TotalMilliseconds;
                int wait = Math.Max(1, NeuralEngine.StepMilliseconds - elapsed);
                Thread.Sleep(wait);
            }
        }

        private void StepLocked()
        {
            _latestSummary = _engine.Step(_liveState, ExternalDrive.Empty);
            _replay.Add(_liveState, _latestSummary);
        }

        public void Dispose()
        {
            _stopping = true;
            Thread thread = _thread;
            if (thread != null && thread.IsAlive)
            {
                if (!thread.Join(1000))
                {
                    thread.Interrupt();
                }
            }
        }
    }
}

