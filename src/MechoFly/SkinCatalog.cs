using System;

namespace MechoFly
{
    internal enum VisualSkin
    {
        Drosophila = 0,
        Firefly = 1
    }

    internal static class SkinCatalog
    {
        public static VisualSkin Default { get { return VisualSkin.Drosophila; } }

        public static VisualSkin ParseRequired(string value)
        {
            if (string.Equals(value, "drosophila", StringComparison.OrdinalIgnoreCase))
            {
                return VisualSkin.Drosophila;
            }
            if (string.Equals(value, "firefly", StringComparison.OrdinalIgnoreCase))
            {
                return VisualSkin.Firefly;
            }
            throw new ArgumentException("Skin must be drosophila or firefly.", "value");
        }

        public static string Key(VisualSkin skin)
        {
            return skin == VisualSkin.Firefly ? "firefly" : "drosophila";
        }

        public static string DisplayName(VisualSkin skin)
        {
            return skin == VisualSkin.Firefly ? "Firefly Prism" : "Drosophila Natural";
        }
    }
}

