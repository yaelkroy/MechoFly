using System;

namespace MechoFly
{
    internal sealed class DeterministicRandom
    {
        private ulong _state;

        public DeterministicRandom(ulong seed)
        {
            _state = seed == 0UL ? 0x9e3779b97f4a7c15UL : seed;
        }

        public uint NextUInt32()
        {
            ulong value = _state;
            value ^= value >> 12;
            value ^= value << 25;
            value ^= value >> 27;
            _state = value;
            return (uint)(unchecked(value * 2685821657736338717UL) >> 32);
        }

        public int NextInt(int exclusiveMaximum)
        {
            if (exclusiveMaximum <= 0)
            {
                throw new ArgumentOutOfRangeException("exclusiveMaximum");
            }
            return (int)(NextUInt32() % (uint)exclusiveMaximum);
        }

        public float NextUnit()
        {
            return (NextUInt32() & 0x00ffffffU) / 16777216.0f;
        }
    }
}
