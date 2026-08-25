using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace MechoFly
{
    internal sealed class FlyOverlayForm : Form
    {
        private readonly SimulationCoordinator _coordinator;
        private readonly Timer _timer;
        private float _phase;
        private int _velocityX;
        private int _velocityY;

        public FlyOverlayForm(SimulationCoordinator coordinator)
        {
            _coordinator = coordinator;
            _velocityX = 2;
            _velocityY = 1;
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            TopMost = true;
            BackColor = Color.Magenta;
            TransparencyKey = Color.Magenta;
            ClientSize = new Size(210, 150);
            StartPosition = FormStartPosition.Manual;
            Rectangle working = Screen.PrimaryScreen.WorkingArea;
            Location = new Point(working.Left + 70, working.Top + 70);
            DoubleBuffered = true;
            _timer = new Timer();
            _timer.Interval = 33;
            _timer.Tick += Tick;
            _timer.Start();
        }

        protected override CreateParams CreateParams
        {
            get
            {
                const int WsExTransparent = 0x20;
                const int WsExToolWindow = 0x80;
                const int WsExLayered = 0x80000;
                CreateParams parameters = base.CreateParams;
                parameters.ExStyle |= WsExTransparent | WsExToolWindow | WsExLayered;
                return parameters;
            }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        protected override void Dispose(bool disposing)
        {
            if (disposing && _timer != null) _timer.Dispose();
            base.Dispose(disposing);
        }

        private void Tick(object sender, EventArgs eventArgs)
        {
            NeuralFrame frame = _coordinator.GetLatestFrame();
            string behavior = frame.Summary.Behavior;
            float speed = behavior == "flight" ? 2.8f : behavior == "walking" ? 1.5f : 0.35f;
            _phase += 0.13f * speed;
            Rectangle working = Screen.FromControl(this).WorkingArea;
            Point next = new Point(Left + (int)(_velocityX * speed), Top + (int)(_velocityY * speed));
            if (next.X < working.Left || next.X + Width > working.Right)
            {
                _velocityX = -_velocityX;
                next.X = Math.Max(working.Left, Math.Min(working.Right - Width, next.X));
            }
            if (next.Y < working.Top || next.Y + Height > working.Bottom)
            {
                _velocityY = -_velocityY;
                next.Y = Math.Max(working.Top, Math.Min(working.Bottom - Height, next.Y));
            }
            Location = next;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.Clear(TransparencyKey);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.TranslateTransform(ClientSize.Width * 0.5f, ClientSize.Height * 0.5f);
            float wing = (float)Math.Sin(_phase) * 13.0f;

            using (Brush wingBrush = new SolidBrush(Color.FromArgb(150, 79, 214, 232)))
            using (Pen wingEdge = new Pen(Color.FromArgb(210, 166, 244, 255), 2.0f))
            {
                PointF[] leftWing = new PointF[]
                {
                    new PointF(-10, -8), new PointF(-72, -42 - wing),
                    new PointF(-88, -4 - wing), new PointF(-18, 10)
                };
                PointF[] rightWing = new PointF[]
                {
                    new PointF(10, -8), new PointF(72, -42 + wing),
                    new PointF(88, -4 + wing), new PointF(18, 10)
                };
                e.Graphics.FillPolygon(wingBrush, leftWing);
                e.Graphics.FillPolygon(wingBrush, rightWing);
                e.Graphics.DrawPolygon(wingEdge, leftWing);
                e.Graphics.DrawPolygon(wingEdge, rightWing);
            }
            using (LinearGradientBrush body = new LinearGradientBrush(
                new Rectangle(-18, -38, 36, 82),
                Color.FromArgb(42, 38, 62),
                Color.FromArgb(30, 214, 182),
                90.0f))
            using (Pen outline = new Pen(Color.FromArgb(190, 245, 221), 2.0f))
            {
                e.Graphics.FillEllipse(body, -18, -34, 36, 70);
                e.Graphics.DrawEllipse(outline, -18, -34, 36, 70);
            }
            using (Brush eye = new SolidBrush(Color.FromArgb(251, 84, 172)))
            {
                e.Graphics.FillEllipse(eye, -15, -39, 11, 11);
                e.Graphics.FillEllipse(eye, 4, -39, 11, 11);
            }
            using (Pen leg = new Pen(Color.FromArgb(124, 234, 203), 2.0f))
            {
                int y;
                for (y = -12; y <= 18; y += 15)
                {
                    e.Graphics.DrawLine(leg, -12, y, -44, y + 20);
                    e.Graphics.DrawLine(leg, 12, y, 44, y + 20);
                }
            }
        }
    }
}

