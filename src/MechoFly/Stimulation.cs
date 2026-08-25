using System;
using System.Collections.Generic;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace MechoFly
{
    internal sealed class StimulationPolicyException : Exception
    {
        public StimulationPolicyException(string message) : base(message)
        {
        }
    }

    internal sealed class StimulationPlan
    {
        public const string RequiredSource = "user_authored_preview";
        public const string PolicyVersion = "mechofly-preview-policy-1";
        public const int MaximumTargets = 64;
        public const float MaximumAmplitude = 0.25f;
        public const int MinimumDurationMilliseconds = 33;
        public const int MaximumDurationMilliseconds = 990;
        public const double MaximumDosage = 8000.0;

        public readonly string PlanId;
        public readonly string Source;
        public readonly string AuthoredBy;
        public readonly int[] Targets;
        public readonly float Amplitude;
        public readonly int DurationMilliseconds;
        public readonly bool PreviewOnly;

        private StimulationPlan(
            string planId,
            string source,
            string authoredBy,
            int[] targets,
            float amplitude,
            int durationMilliseconds,
            bool previewOnly)
        {
            PlanId = planId;
            Source = source;
            AuthoredBy = authoredBy;
            Targets = targets;
            Amplitude = amplitude;
            DurationMilliseconds = durationMilliseconds;
            PreviewOnly = previewOnly;
        }

        public static StimulationPlan CreateAuthored(
            string authoredBy,
            IList<int> targets,
            float amplitude,
            int durationMilliseconds,
            int neuronCount)
        {
            if (string.IsNullOrWhiteSpace(authoredBy))
            {
                throw new StimulationPolicyException("An explicit plan author is required.");
            }
            if (targets == null)
            {
                throw new StimulationPolicyException("At least one target is required.");
            }

            SortedSet<int> unique = new SortedSet<int>();
            int i;
            for (i = 0; i < targets.Count; i++)
            {
                if (targets[i] < 0 || targets[i] >= neuronCount)
                {
                    throw new StimulationPolicyException("A target is outside the modeled neuron range.");
                }
                unique.Add(targets[i]);
            }
            int[] normalized = new int[unique.Count];
            unique.CopyTo(normalized);

            StimulationPlan plan = new StimulationPlan(
                "preview-" + Guid.NewGuid().ToString("N"),
                RequiredSource,
                authoredBy.Trim(),
                normalized,
                amplitude,
                durationMilliseconds,
                true);
            plan.Validate(neuronCount);
            return plan;
        }

        public void Validate(int neuronCount)
        {
            if (!string.Equals(Source, RequiredSource, StringComparison.Ordinal))
            {
                throw new StimulationPolicyException("The plan provenance source is not permitted.");
            }
            if (!PreviewOnly)
            {
                throw new StimulationPolicyException("Only preview-only plans are permitted.");
            }
            if (Targets.Length < 1 || Targets.Length > MaximumTargets)
            {
                throw new StimulationPolicyException("Target count exceeds the preview policy.");
            }
            if (float.IsNaN(Amplitude) || float.IsInfinity(Amplitude) ||
                Amplitude <= 0.0f || Amplitude > MaximumAmplitude)
            {
                throw new StimulationPolicyException("Amplitude exceeds the preview policy.");
            }
            if (DurationMilliseconds < MinimumDurationMilliseconds ||
                DurationMilliseconds > MaximumDurationMilliseconds)
            {
                throw new StimulationPolicyException("Duration exceeds the preview policy.");
            }
            double dosage = Amplitude * Targets.Length * DurationMilliseconds;
            if (dosage > MaximumDosage)
            {
                throw new StimulationPolicyException("Aggregate dosage exceeds the preview policy.");
            }
            int i;
            for (i = 0; i < Targets.Length; i++)
            {
                if (Targets[i] < 0 || Targets[i] >= neuronCount)
                {
                    throw new StimulationPolicyException("A target is outside the modeled neuron range.");
                }
                if (i > 0 && Targets[i - 1] >= Targets[i])
                {
                    throw new StimulationPolicyException("Targets must be unique and ordered.");
                }
            }
        }

        public ExternalDrive DriveAtFrame(int frameOffset)
        {
            int activeFrames = (DurationMilliseconds + NeuralEngine.StepMilliseconds - 1) /
                NeuralEngine.StepMilliseconds;
            if (frameOffset < 1 || frameOffset > activeFrames)
            {
                return ExternalDrive.Empty;
            }
            ExternalDrive drive = new ExternalDrive();
            int i;
            for (i = 0; i < Targets.Length; i++)
            {
                drive.Add(Targets[i], Amplitude);
            }
            return drive;
        }

        public string Digest()
        {
            StringBuilder canonical = new StringBuilder();
            canonical.Append(PolicyVersion).Append('|');
            canonical.Append(Source).Append('|');
            canonical.Append(AuthoredBy).Append('|');
            canonical.Append(Amplitude.ToString("R", CultureInfo.InvariantCulture)).Append('|');
            canonical.Append(DurationMilliseconds.ToString(CultureInfo.InvariantCulture)).Append('|');
            int i;
            for (i = 0; i < Targets.Length; i++)
            {
                if (i > 0) canonical.Append(',');
                canonical.Append(Targets[i].ToString(CultureInfo.InvariantCulture));
            }
            using (SHA256 sha = SHA256.Create())
            {
                byte[] bytes = sha.ComputeHash(Encoding.UTF8.GetBytes(canonical.ToString()));
                StringBuilder hex = new StringBuilder(bytes.Length * 2);
                for (i = 0; i < bytes.Length; i++)
                {
                    hex.Append(bytes[i].ToString("x2", CultureInfo.InvariantCulture));
                }
                return hex.ToString();
            }
        }
    }

    internal sealed class StimulationReceipt
    {
        public string Status;
        public string PolicyVersion;
        public string PlanId;
        public string PlanDigest;
        public string Source;
        public string AuthoredBy;
        public string GeneratedUtc;
        public int FrameCount;
        public int TargetCount;
        public float Amplitude;
        public int DurationMilliseconds;
        public string LiveStateBefore;
        public string LiveStateAfter;
        public bool LiveStateUnchanged;
        public bool PreviewOnly;
        public bool HardwareSideEffects;

        public string ToJson()
        {
            CultureInfo invariant = CultureInfo.InvariantCulture;
            StringBuilder json = new StringBuilder();
            json.Append("{\n");
            AppendString(json, "status", Status, true);
            AppendString(json, "policy_version", PolicyVersion, true);
            AppendString(json, "plan_id", PlanId, true);
            AppendString(json, "plan_digest", PlanDigest, true);
            AppendString(json, "source", Source, true);
            AppendString(json, "authored_by", AuthoredBy, true);
            AppendString(json, "generated_utc", GeneratedUtc, true);
            json.Append("  \"frame_count\": ").Append(FrameCount.ToString(invariant)).Append(",\n");
            json.Append("  \"target_count\": ").Append(TargetCount.ToString(invariant)).Append(",\n");
            json.Append("  \"amplitude\": ").Append(Amplitude.ToString("R", invariant)).Append(",\n");
            json.Append("  \"duration_ms\": ").Append(DurationMilliseconds.ToString(invariant)).Append(",\n");
            AppendString(json, "live_state_before", LiveStateBefore, true);
            AppendString(json, "live_state_after", LiveStateAfter, true);
            json.Append("  \"live_state_unchanged\": ").Append(LiveStateUnchanged ? "true" : "false").Append(",\n");
            json.Append("  \"preview_only\": ").Append(PreviewOnly ? "true" : "false").Append(",\n");
            json.Append("  \"hardware_side_effects\": ").Append(HardwareSideEffects ? "true" : "false").Append("\n");
            json.Append("}\n");
            return json.ToString();
        }

        private static void AppendString(StringBuilder json, string name, string value, bool comma)
        {
            json.Append("  \"").Append(Escape(name)).Append("\": \"");
            json.Append(Escape(value ?? string.Empty)).Append('"');
            if (comma) json.Append(',');
            json.Append('\n');
        }

        private static string Escape(string value)
        {
            return value.Replace("\\", "\\\\").Replace("\"", "\\\"")
                .Replace("\r", "\\r").Replace("\n", "\\n");
        }
    }
}

