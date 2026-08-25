using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;

namespace MechoFly
{
    internal static class SelfTest
    {
        public static int Run(string receiptPath)
        {
            bool liveStateUnchanged = false;
            bool alternativeDiffers = false;
            bool excessivePlanRejected = false;
            bool replayBounded = false;
            bool defaultSkinDrosophila = SkinCatalog.Default == VisualSkin.Drosophila;
            bool drosophilaSkinAvailable = false;
            bool fireflySkinAvailable = false;
            bool invalidSkinRejected = false;
            int frameCount = 0;
            string failure = string.Empty;
            string previewReceipt = "{}";

            try
            {
                drosophilaSkinAvailable = SkinCatalog.ParseRequired("drosophila") == VisualSkin.Drosophila;
                fireflySkinAvailable = SkinCatalog.ParseRequired("firefly") == VisualSkin.Firefly;
                try
                {
                    SkinCatalog.ParseRequired("unknown");
                }
                catch (ArgumentException)
                {
                    invalidSkinRejected = true;
                }

                using (SimulationCoordinator coordinator = new SimulationCoordinator(false))
                {
                    int i;
                    for (i = 0; i < 300; i++)
                    {
                        coordinator.StepForTest();
                    }
                    replayBounded = coordinator.GetReplayCount() == BoundedReplayStore.MaximumFrames;

                    List<int> targets = new List<int>();
                    targets.Add(3);
                    targets.Add(7);
                    targets.Add(11);
                    targets.Add(19);
                    targets.Add(31);
                    StimulationPlan plan = StimulationPlan.CreateAuthored(
                        "deterministic self-test",
                        targets,
                        0.24f,
                        330,
                        coordinator.Engine.NeuronCount);
                    ComparisonSequence sequence = coordinator.BuildPreview(plan, 90);
                    frameCount = sequence.Frames.Length;
                    liveStateUnchanged = sequence.Receipt.LiveStateUnchanged;
                    alternativeDiffers = ComparisonBuilder.HasAnyDifference(sequence.Frames);
                    previewReceipt = sequence.Receipt.ToJson().Trim();

                    try
                    {
                        StimulationPlan.CreateAuthored(
                            "deterministic self-test",
                            targets,
                            0.251f,
                            330,
                            coordinator.Engine.NeuronCount);
                    }
                    catch (StimulationPolicyException)
                    {
                        excessivePlanRejected = true;
                    }
                }
            }
            catch (Exception exception)
            {
                failure = exception.GetType().Name + ": " + exception.Message;
            }

            bool passed = liveStateUnchanged && alternativeDiffers && excessivePlanRejected &&
                replayBounded && frameCount == 90 && defaultSkinDrosophila &&
                drosophilaSkinAvailable && fireflySkinAvailable && invalidSkinRejected &&
                string.IsNullOrEmpty(failure);
            string json = BuildReceipt(
                passed,
                liveStateUnchanged,
                alternativeDiffers,
                excessivePlanRejected,
                replayBounded,
                defaultSkinDrosophila,
                drosophilaSkinAvailable,
                fireflySkinAvailable,
                invalidSkinRejected,
                frameCount,
                failure,
                previewReceipt);

            try
            {
                string fullPath = Path.GetFullPath(receiptPath);
                string directory = Path.GetDirectoryName(fullPath);
                if (!Directory.Exists(directory)) Directory.CreateDirectory(directory);
                File.WriteAllText(fullPath, json, new UTF8Encoding(false));
            }
            catch
            {
                return 2;
            }
            return passed ? 0 : 1;
        }

        private static string BuildReceipt(
            bool passed,
            bool liveStateUnchanged,
            bool alternativeDiffers,
            bool excessivePlanRejected,
            bool replayBounded,
            bool defaultSkinDrosophila,
            bool drosophilaSkinAvailable,
            bool fireflySkinAvailable,
            bool invalidSkinRejected,
            int frameCount,
            string failure,
            string previewReceipt)
        {
            StringBuilder json = new StringBuilder();
            json.Append("{\n");
            json.Append("  \"status\": \"").Append(passed ? "PASS" : "FAIL").Append("\",\n");
            json.Append("  \"live_state_unchanged\": ").Append(liveStateUnchanged ? "true" : "false").Append(",\n");
            json.Append("  \"alternative_differs\": ").Append(alternativeDiffers ? "true" : "false").Append(",\n");
            json.Append("  \"excessive_plan_rejected\": ").Append(excessivePlanRejected ? "true" : "false").Append(",\n");
            json.Append("  \"replay_bounded\": ").Append(replayBounded ? "true" : "false").Append(",\n");
            json.Append("  \"default_skin\": \"drosophila\",\n");
            json.Append("  \"default_skin_is_drosophila\": ").Append(defaultSkinDrosophila ? "true" : "false").Append(",\n");
            json.Append("  \"drosophila_skin_available\": ").Append(drosophilaSkinAvailable ? "true" : "false").Append(",\n");
            json.Append("  \"firefly_skin_available\": ").Append(fireflySkinAvailable ? "true" : "false").Append(",\n");
            json.Append("  \"invalid_skin_rejected\": ").Append(invalidSkinRejected ? "true" : "false").Append(",\n");
            json.Append("  \"comparison_frames\": ").Append(frameCount.ToString(CultureInfo.InvariantCulture)).Append(",\n");
            json.Append("  \"failure\": \"").Append(Escape(failure)).Append("\",\n");
            json.Append("  \"preview_receipt\": ").Append(previewReceipt).Append('\n');
            json.Append("}\n");
            return json.ToString();
        }

        private static string Escape(string value)
        {
            return (value ?? string.Empty).Replace("\\", "\\\\").Replace("\"", "\\\"")
                .Replace("\r", "\\r").Replace("\n", "\\n");
        }
    }
}
