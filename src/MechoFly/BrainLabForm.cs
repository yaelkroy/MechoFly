using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Text;
using System.Windows.Forms;

namespace MechoFly
{
    internal sealed class BrainLabForm : Form
    {
        private readonly SimulationCoordinator _coordinator;
        private readonly BrainPlotControl _actualPlot;
        private readonly BrainPlotControl _alternativePlot;
        private readonly TextBox _targets;
        private readonly NumericUpDown _amplitude;
        private readonly NumericUpDown _duration;
        private readonly NumericUpDown _frames;
        private readonly TrackBar _timeline;
        private readonly TextBox _receipt;
        private readonly Label _status;
        private readonly Button _play;
        private readonly Timer _timer;
        private ComparisonSequence _comparison;
        private bool _playing;

        public BrainLabForm(SimulationCoordinator coordinator)
        {
            _coordinator = coordinator;
            Text = "MechoFly Brain Lab — bounded modeled replay";
            BackColor = Color.FromArgb(6, 12, 22);
            ForeColor = Color.White;
            Font = new Font("Segoe UI", 9.0f);
            StartPosition = FormStartPosition.CenterScreen;
            MinimumSize = new Size(980, 680);
            ClientSize = new Size(1220, 790);

            TableLayoutPanel root = new TableLayoutPanel();
            root.Dock = DockStyle.Fill;
            root.Padding = new Padding(12);
            root.RowCount = 5;
            root.ColumnCount = 1;
            root.RowStyles.Add(new RowStyle(SizeType.Absolute, 68.0f));
            root.RowStyles.Add(new RowStyle(SizeType.Percent, 100.0f));
            root.RowStyles.Add(new RowStyle(SizeType.Absolute, 82.0f));
            root.RowStyles.Add(new RowStyle(SizeType.Absolute, 56.0f));
            root.RowStyles.Add(new RowStyle(SizeType.Absolute, 112.0f));
            Controls.Add(root);

            Label header = new Label();
            header.Dock = DockStyle.Fill;
            header.Text = "MECHOFLY  •  BRAIN LAB\r\nMODELED DYNAMICS  •  SYNTHETIC DEMO TOPOLOGY  •  PREVIEW ONLY";
            header.Font = new Font("Segoe UI", 15.0f, FontStyle.Bold);
            header.ForeColor = Color.FromArgb(105, 220, 255);
            header.TextAlign = ContentAlignment.MiddleLeft;
            root.Controls.Add(header, 0, 0);

            TableLayoutPanel pair = new TableLayoutPanel();
            pair.Dock = DockStyle.Fill;
            pair.ColumnCount = 2;
            pair.RowCount = 1;
            pair.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50.0f));
            pair.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50.0f));
            root.Controls.Add(pair, 0, 1);
            _actualPlot = new BrainPlotControl(coordinator.Engine, "ACTUAL • RECORDED MODELED FRAME", Color.FromArgb(74, 206, 255));
            _alternativePlot = new BrainPlotControl(coordinator.Engine, "ALTERNATIVE • NO PREVIEW YET", Color.FromArgb(255, 187, 74));
            _actualPlot.Dock = DockStyle.Fill;
            _alternativePlot.Dock = DockStyle.Fill;
            _actualPlot.Margin = new Padding(0, 0, 6, 0);
            _alternativePlot.Margin = new Padding(6, 0, 0, 0);
            pair.Controls.Add(_actualPlot, 0, 0);
            pair.Controls.Add(_alternativePlot, 1, 0);

            FlowLayoutPanel authoring = new FlowLayoutPanel();
            authoring.Dock = DockStyle.Fill;
            authoring.WrapContents = true;
            authoring.Padding = new Padding(0, 10, 0, 0);
            root.Controls.Add(authoring, 0, 2);
            authoring.Controls.Add(FieldLabel("Targets"));
            _targets = new TextBox();
            _targets.Width = 185;
            _targets.Text = "3, 7, 11, 19, 31";
            authoring.Controls.Add(_targets);
            authoring.Controls.Add(FieldLabel("Amplitude"));
            _amplitude = Numeric(0.01m, 0.25m, 0.01m, 0.20m, 2, 78);
            authoring.Controls.Add(_amplitude);
            authoring.Controls.Add(FieldLabel("Duration ms"));
            _duration = Numeric(33m, 990m, 33m, 330m, 0, 84);
            authoring.Controls.Add(_duration);
            authoring.Controls.Add(FieldLabel("Frames"));
            _frames = Numeric(30m, 120m, 1m, 90m, 0, 68);
            authoring.Controls.Add(_frames);
            Button generate = new Button();
            generate.Text = "Generate preview";
            generate.AutoSize = true;
            generate.Height = 30;
            generate.BackColor = Color.FromArgb(19, 111, 146);
            generate.ForeColor = Color.White;
            generate.FlatStyle = FlatStyle.Flat;
            generate.Click += GenerateClick;
            authoring.Controls.Add(generate);

            TableLayoutPanel transport = new TableLayoutPanel();
            transport.Dock = DockStyle.Fill;
            transport.ColumnCount = 3;
            transport.RowCount = 1;
            transport.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 86.0f));
            transport.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100.0f));
            transport.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 370.0f));
            root.Controls.Add(transport, 0, 3);
            _play = new Button();
            _play.Text = "Play";
            _play.Enabled = false;
            _play.Dock = DockStyle.Fill;
            _play.Click += PlayClick;
            transport.Controls.Add(_play, 0, 0);
            _timeline = new TrackBar();
            _timeline.Dock = DockStyle.Fill;
            _timeline.Minimum = 0;
            _timeline.Maximum = 1;
            _timeline.TickStyle = TickStyle.None;
            _timeline.Enabled = false;
            _timeline.ValueChanged += TimelineChanged;
            transport.Controls.Add(_timeline, 1, 0);
            _status = new Label();
            _status.Dock = DockStyle.Fill;
            _status.TextAlign = ContentAlignment.MiddleLeft;
            _status.ForeColor = Color.FromArgb(169, 200, 218);
            _status.Text = "Collecting bounded replay…";
            transport.Controls.Add(_status, 2, 0);

            _receipt = new TextBox();
            _receipt.Dock = DockStyle.Fill;
            _receipt.Multiline = true;
            _receipt.ReadOnly = true;
            _receipt.ScrollBars = ScrollBars.Vertical;
            _receipt.BackColor = Color.FromArgb(9, 19, 32);
            _receipt.ForeColor = Color.FromArgb(174, 224, 193);
            _receipt.Font = new Font("Consolas", 8.5f);
            _receipt.Text = "No preview receipt. Live state cannot be targeted from this UI.";
            root.Controls.Add(_receipt, 0, 4);

            _timer = new Timer();
            _timer.Interval = 100;
            _timer.Tick += TimerTick;
            _timer.Start();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing && _timer != null) _timer.Dispose();
            base.Dispose(disposing);
        }

        private static Label FieldLabel(string text)
        {
            Label label = new Label();
            label.Text = text;
            label.AutoSize = true;
            label.Margin = new Padding(8, 7, 4, 0);
            label.ForeColor = Color.FromArgb(180, 211, 228);
            return label;
        }

        private static NumericUpDown Numeric(
            decimal minimum,
            decimal maximum,
            decimal increment,
            decimal value,
            int decimalPlaces,
            int width)
        {
            NumericUpDown numeric = new NumericUpDown();
            numeric.Minimum = minimum;
            numeric.Maximum = maximum;
            numeric.Increment = increment;
            numeric.Value = value;
            numeric.DecimalPlaces = decimalPlaces;
            numeric.Width = width;
            return numeric;
        }

        private void GenerateClick(object sender, EventArgs eventArgs)
        {
            try
            {
                List<int> targets = ParseTargets(_targets.Text);
                StimulationPlan plan = StimulationPlan.CreateAuthored(
                    "Brain Lab operator",
                    targets,
                    (float)_amplitude.Value,
                    (int)_duration.Value,
                    _coordinator.Engine.NeuronCount);
                _comparison = _coordinator.BuildPreview(plan, (int)_frames.Value);
                _timeline.Minimum = 0;
                _timeline.Maximum = _comparison.Frames.Length - 1;
                _timeline.Value = 0;
                _timeline.Enabled = true;
                _play.Enabled = true;
                _playing = false;
                _play.Text = "Play";
                _receipt.Text = _comparison.Receipt.ToJson();
                ShowComparisonFrame(0);
            }
            catch (Exception exception)
            {
                _playing = false;
                _status.Text = "REJECTED • " + exception.Message;
                _status.ForeColor = Color.FromArgb(255, 116, 116);
            }
        }

        private static List<int> ParseTargets(string text)
        {
            string[] fields = (text ?? string.Empty).Split(new char[] { ',', ';', ' ' },
                StringSplitOptions.RemoveEmptyEntries);
            List<int> values = new List<int>();
            int i;
            for (i = 0; i < fields.Length; i++)
            {
                int value;
                if (!int.TryParse(fields[i], NumberStyles.Integer, CultureInfo.InvariantCulture, out value))
                {
                    throw new StimulationPolicyException("Targets must be integer model indices.");
                }
                values.Add(value);
            }
            return values;
        }

        private void PlayClick(object sender, EventArgs eventArgs)
        {
            if (_comparison == null) return;
            _playing = !_playing;
            _play.Text = _playing ? "Pause" : "Play";
        }

        private void TimelineChanged(object sender, EventArgs eventArgs)
        {
            if (_comparison != null) ShowComparisonFrame(_timeline.Value);
        }

        private void TimerTick(object sender, EventArgs eventArgs)
        {
            if (_comparison == null)
            {
                NeuralFrame frame = _coordinator.GetLatestFrame();
                _actualPlot.SetFrame(frame, "ACTUAL • LIVE MODELED FRAME");
                _alternativePlot.SetFrame(null, "ALTERNATIVE • AUTHOR A PREVIEW TO COMPARE");
                _status.Text = string.Format(
                    CultureInfo.InvariantCulture,
                    "replay {0}/{1} frames • live state isolated",
                    _coordinator.GetReplayCount(),
                    BoundedReplayStore.MaximumFrames);
                return;
            }
            if (_playing)
            {
                int next = _timeline.Value + 1;
                if (next > _timeline.Maximum) next = _timeline.Minimum;
                _timeline.Value = next;
            }
        }

        private void ShowComparisonFrame(int index)
        {
            ComparisonFrame frame = _comparison.Frames[index];
            _actualPlot.SetFrame(frame.Actual, "ACTUAL • BOUNDED REPLAY");
            _alternativePlot.SetFrame(frame.Alternative, "ALTERNATIVE • AUTHORED PREVIEW");
            _status.ForeColor = Color.FromArgb(174, 224, 193);
            _status.Text = string.Format(
                CultureInfo.InvariantCulture,
                "frame {0}/{1} • +{2} ms • live digest unchanged: {3}",
                index + 1,
                _comparison.Frames.Length,
                frame.OffsetMilliseconds,
                _comparison.Receipt.LiveStateUnchanged ? "YES" : "NO");
        }
    }
}

