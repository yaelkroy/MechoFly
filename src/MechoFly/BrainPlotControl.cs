using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace MechoFly
{
    internal sealed class BrainPlotControl : Control
    {
        private readonly NeuralEngine _engine;
        private NeuralFrame _frame;
        private string _caption;
        private Color _accent;

        public BrainPlotControl(NeuralEngine engine, string caption, Color accent)
        {
            _engine = engine;
            _caption = caption;
            _accent = accent;
            SetStyle(ControlStyles.AllPaintingInWmPaint |
                ControlStyles.UserPaint |
                ControlStyles.OptimizedDoubleBuffer |
                ControlStyles.ResizeRedraw, true);
            BackColor = Color.FromArgb(7, 14, 25);
            ForeColor = Color.White;
            MinimumSize = new Size(300, 300);
        }

        public void SetFrame(NeuralFrame frame, string caption)
        {
            _frame = frame;
            if (!string.IsNullOrEmpty(caption)) _caption = caption;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            Graphics graphics = e.Graphics;
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.Clear(BackColor);

            Rectangle plot = new Rectangle(16, 48, Math.Max(1, Width - 32), Math.Max(1, Height - 92));
            using (Pen border = new Pen(Color.FromArgb(55, 103, 137)))
            {
                graphics.DrawRectangle(border, plot);
            }
            using (Font title = new Font("Segoe UI", 11.0f, FontStyle.Bold))
            using (Brush brush = new SolidBrush(_accent))
            {
                graphics.DrawString(_caption, title, brush, 16.0f, 14.0f);
            }

            NeuralFrame frame = _frame;
            if (frame == null)
            {
                using (Brush brush = new SolidBrush(Color.FromArgb(150, 175, 195)))
                {
                    graphics.DrawString("Waiting for modeled frames…", Font, brush, 28.0f, 76.0f);
                }
                return;
            }

            NeuronPoint[] points = _engine.Points;
            int i;
            for (i = 0; i < points.Length; i++)
            {
                float px = plot.Left + plot.Width * (points[i].X + 1.0f) * 0.5f;
                float py = plot.Top + plot.Height * (points[i].Y + 1.0f) * 0.5f;
                bool spike = frame.State.Spiked[i];
                float potential = Math.Max(0.0f, Math.Min(1.0f, frame.State.Potential[i]));
                Color color = spike
                    ? Color.FromArgb(255, 242, 72)
                    : GroupColor(points[i].Group, 55 + (int)(potential * 135.0f));
                float size = spike ? 5.5f : 2.2f;
                using (Brush dot = new SolidBrush(color))
                {
                    graphics.FillEllipse(dot, px - size * 0.5f, py - size * 0.5f, size, size);
                }
            }

            string footer = string.Format(
                System.Globalization.CultureInfo.InvariantCulture,
                "frame {0}  •  {1} spikes  •  {2}  •  SYNTHETIC MODEL",
                frame.Summary.StepIndex,
                frame.Summary.SpikeCount,
                frame.Summary.Behavior.ToUpperInvariant());
            using (Brush brush = new SolidBrush(Color.FromArgb(156, 190, 211)))
            {
                graphics.DrawString(footer, Font, brush, 16.0f, Height - 32.0f);
            }
        }

        private static Color GroupColor(byte group, int alpha)
        {
            Color[] colors = new Color[]
            {
                Color.FromArgb(40, 210, 202),
                Color.FromArgb(76, 150, 240),
                Color.FromArgb(163, 90, 225),
                Color.FromArgb(236, 83, 185),
                Color.FromArgb(63, 194, 123),
                Color.FromArgb(244, 156, 61),
                Color.FromArgb(174, 218, 76),
                Color.FromArgb(88, 204, 238),
                Color.FromArgb(250, 207, 74)
            };
            Color source = colors[group % colors.Length];
            return Color.FromArgb(alpha, source.R, source.G, source.B);
        }
    }
}

