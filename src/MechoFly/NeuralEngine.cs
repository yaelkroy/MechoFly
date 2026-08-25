using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace MechoFly
{
    internal struct NeuronPoint
    {
        public float X;
        public float Y;
        public byte Group;

        public NeuronPoint(float x, float y, byte group)
        {
            X = x;
            Y = y;
            Group = group;
        }
    }

    internal sealed class ExternalDrive
    {
        private readonly Dictionary<int, float> _values;

        public static readonly ExternalDrive Empty = new ExternalDrive();

        public ExternalDrive()
        {
            _values = new Dictionary<int, float>();
        }

        public void Add(int neuronIndex, float value)
        {
            float existing;
            if (_values.TryGetValue(neuronIndex, out existing))
            {
                _values[neuronIndex] = existing + value;
            }
            else
            {
                _values.Add(neuronIndex, value);
            }
        }

        public float Get(int neuronIndex)
        {
            float value;
            return _values.TryGetValue(neuronIndex, out value) ? value : 0.0f;
        }
    }

    internal sealed class NeuralState
    {
        public ulong StepIndex;
        public readonly float[] Potential;
        public readonly byte[] Refractory;
        public readonly bool[] Spiked;

        public NeuralState(int neuronCount)
        {
            if (neuronCount <= 0)
            {
                throw new ArgumentOutOfRangeException("neuronCount");
            }
            Potential = new float[neuronCount];
            Refractory = new byte[neuronCount];
            Spiked = new bool[neuronCount];
        }

        public NeuralState CloneDeep()
        {
            NeuralState copy = new NeuralState(Potential.Length);
            copy.StepIndex = StepIndex;
            Array.Copy(Potential, copy.Potential, Potential.Length);
            Array.Copy(Refractory, copy.Refractory, Refractory.Length);
            Array.Copy(Spiked, copy.Spiked, Spiked.Length);
            return copy;
        }

        public string Digest()
        {
            using (MemoryStream stream = new MemoryStream())
            using (BinaryWriter writer = new BinaryWriter(stream, Encoding.UTF8))
            {
                writer.Write(StepIndex);
                int i;
                for (i = 0; i < Potential.Length; i++)
                {
                    writer.Write(Potential[i]);
                    writer.Write(Refractory[i]);
                    writer.Write(Spiked[i]);
                }
                writer.Flush();
                using (SHA256 sha = SHA256.Create())
                {
                    return Hex(sha.ComputeHash(stream.ToArray()));
                }
            }
        }

        private static string Hex(byte[] bytes)
        {
            StringBuilder builder = new StringBuilder(bytes.Length * 2);
            int i;
            for (i = 0; i < bytes.Length; i++)
            {
                builder.Append(bytes[i].ToString("x2", CultureInfo.InvariantCulture));
            }
            return builder.ToString();
        }
    }

    internal sealed class NeuralSummary
    {
        public ulong StepIndex;
        public int SpikeCount;
        public readonly float[] GroupRates;
        public string Behavior;

        public NeuralSummary(int groupCount)
        {
            GroupRates = new float[groupCount];
            Behavior = "rest";
        }

        public NeuralSummary CloneValue()
        {
            NeuralSummary copy = new NeuralSummary(GroupRates.Length);
            copy.StepIndex = StepIndex;
            copy.SpikeCount = SpikeCount;
            copy.Behavior = Behavior;
            Array.Copy(GroupRates, copy.GroupRates, GroupRates.Length);
            return copy;
        }
    }

    internal sealed class NeuralFrame
    {
        public readonly NeuralState State;
        public readonly NeuralSummary Summary;

        public NeuralFrame(NeuralState state, NeuralSummary summary)
        {
            State = state;
            Summary = summary;
        }

        public NeuralFrame CloneDeep()
        {
            return new NeuralFrame(State.CloneDeep(), Summary.CloneValue());
        }
    }

    internal sealed class NeuralEngine
    {
        public const int StepMilliseconds = 33;
        public const int GroupCount = 9;

        private readonly int[] _edgeSource;
        private readonly int[] _edgeTarget;
        private readonly float[] _edgeWeight;
        private readonly int[] _groupSizes;
        private readonly NeuronPoint[] _points;

        public int NeuronCount { get { return _points.Length; } }
        public int EdgeCount { get { return _edgeSource.Length; } }
        public NeuronPoint[] Points { get { return _points; } }

        private NeuralEngine(
            NeuronPoint[] points,
            int[] edgeSource,
            int[] edgeTarget,
            float[] edgeWeight)
        {
            _points = points;
            _edgeSource = edgeSource;
            _edgeTarget = edgeTarget;
            _edgeWeight = edgeWeight;
            _groupSizes = new int[GroupCount];
            int i;
            for (i = 0; i < points.Length; i++)
            {
                _groupSizes[points[i].Group]++;
            }
        }

        public static NeuralEngine CreateSyntheticDemo(int neuronCount, int outgoingEdges, ulong seed)
        {
            if (neuronCount < 128 || neuronCount > 20000)
            {
                throw new ArgumentOutOfRangeException("neuronCount");
            }
            if (outgoingEdges < 2 || outgoingEdges > 32)
            {
                throw new ArgumentOutOfRangeException("outgoingEdges");
            }

            DeterministicRandom random = new DeterministicRandom(seed);
            NeuronPoint[] points = new NeuronPoint[neuronCount];
            int i;
            for (i = 0; i < neuronCount; i++)
            {
                bool left = (i & 1) == 0;
                double angle = random.NextUnit() * Math.PI * 2.0;
                double radius = Math.Sqrt(random.NextUnit());
                float x = (left ? -0.43f : 0.43f) + (float)(Math.Cos(angle) * radius * 0.48);
                float y = (float)(Math.Sin(angle) * radius * 0.72);
                points[i] = new NeuronPoint(x, y, (byte)(i % GroupCount));
            }

            int edgeCount = checked(neuronCount * outgoingEdges);
            int[] source = new int[edgeCount];
            int[] target = new int[edgeCount];
            float[] weight = new float[edgeCount];
            int edge = 0;
            for (i = 0; i < neuronCount; i++)
            {
                int j;
                for (j = 0; j < outgoingEdges; j++)
                {
                    source[edge] = i;
                    target[edge] = random.NextInt(neuronCount);
                    float sign = (random.NextUInt32() % 5U) == 0U ? -1.0f : 1.0f;
                    weight[edge] = sign * (0.018f + random.NextUnit() * 0.035f);
                    edge++;
                }
            }
            return new NeuralEngine(points, source, target, weight);
        }

        public NeuralState CreateState()
        {
            return new NeuralState(NeuronCount);
        }

        public NeuralSummary Step(NeuralState state, ExternalDrive drive)
        {
            if (state == null || state.Potential.Length != NeuronCount)
            {
                throw new ArgumentException("State does not belong to this engine.", "state");
            }
            if (drive == null)
            {
                drive = ExternalDrive.Empty;
            }

            float[] incoming = new float[NeuronCount];
            int edge;
            for (edge = 0; edge < _edgeSource.Length; edge++)
            {
                if (state.Spiked[_edgeSource[edge]])
                {
                    incoming[_edgeTarget[edge]] += _edgeWeight[edge];
                }
            }

            NeuralSummary summary = new NeuralSummary(GroupCount);
            state.StepIndex++;
            summary.StepIndex = state.StepIndex;
            int i;
            for (i = 0; i < NeuronCount; i++)
            {
                if (state.Refractory[i] > 0)
                {
                    state.Refractory[i]--;
                    state.Potential[i] = 0.08f;
                    state.Spiked[i] = false;
                    continue;
                }

                uint noiseBits = Mix((uint)i, (uint)state.StepIndex);
                float noise = (noiseBits & 1023U) / 1023.0f;
                float pulse = ((state.StepIndex + (ulong)(i * 17)) % 97UL) == 0UL ? 0.38f : 0.0f;
                float potential = state.Potential[i] * 0.925f;
                potential += 0.045f + noise * 0.018f + pulse + incoming[i] + drive.Get(i);
                if (potential >= 1.0f)
                {
                    state.Spiked[i] = true;
                    state.Potential[i] = 0.06f;
                    state.Refractory[i] = 2;
                    summary.SpikeCount++;
                    summary.GroupRates[_points[i].Group] += 1.0f;
                }
                else
                {
                    state.Spiked[i] = false;
                    state.Potential[i] = Clamp(potential, -0.25f, 1.2f);
                }
            }

            for (i = 0; i < GroupCount; i++)
            {
                summary.GroupRates[i] = _groupSizes[i] == 0
                    ? 0.0f
                    : summary.GroupRates[i] * 1000.0f / (StepMilliseconds * _groupSizes[i]);
            }
            summary.Behavior = SelectBehavior(summary.GroupRates);
            return summary;
        }

        private static string SelectBehavior(float[] rates)
        {
            if (rates[7] > 8.0f) return "flight";
            if (rates[8] > 7.0f) return "landing";
            if (rates[3] > 6.0f) return "reverse";
            if (rates[6] > 6.0f) return "grooming";
            if (rates[4] > 4.0f) return "walking";
            return "rest";
        }

        private static uint Mix(uint a, uint b)
        {
            unchecked
            {
                uint x = a * 0x9e3779b9U + b * 0x85ebca6bU + 0xc2b2ae35U;
                x ^= x >> 16;
                x *= 0x7feb352dU;
                x ^= x >> 15;
                x *= 0x846ca68bU;
                return x ^ (x >> 16);
            }
        }

        private static float Clamp(float value, float minimum, float maximum)
        {
            if (value < minimum) return minimum;
            if (value > maximum) return maximum;
            return value;
        }
    }
}
