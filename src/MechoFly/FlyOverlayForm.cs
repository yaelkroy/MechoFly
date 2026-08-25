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
        private string _behavior;
        private VisualSkin _skin;

        public FlyOverlayForm(SimulationCoordinator coordinator, VisualSkin initialSkin)
        {
            _coordinator = coordinator;
            _skin = initialSkin;
            _behavior = "rest";
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

        public VisualSkin Skin { get { return _skin; } }

        public void SetSkin(VisualSkin skin)
        {
            _skin = skin;
            Text = "MechoFly — " + SkinCatalog.DisplayName(skin);
            Invalidate();
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
            _behavior = behavior;
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
            if (_skin == VisualSkin.Firefly)
            {
                DrawFirefly(e.Graphics);
            }
            else
            {
                DrawDrosophila(e.Graphics);
            }
        }

        private void DrawDrosophila(Graphics graphics)
        {
            float wing = _behavior == "flight" ? (float)Math.Sin(_phase) * 12.0f : 0.0f;
            using (Brush shadow = new SolidBrush(Color.FromArgb(55, 0, 0, 0)))
            {
                graphics.FillEllipse(shadow, -67, 43, 134, 18);
            }
            using (Brush wingBrush = new SolidBrush(Color.FromArgb(128, 196, 225, 232)))
            using (Pen wingEdge = new Pen(Color.FromArgb(205, 226, 246, 244), 1.8f))
            {
                PointF[] upper = _behavior == "flight"
                    ? new PointF[]
                    {
                        new PointF(-5, -13), new PointF(34, -58 - wing),
                        new PointF(88, -43 - wing), new PointF(28, 4)
                    }
                    : new PointF[]
                    {
                        new PointF(-3, -10), new PointF(56, -25),
                        new PointF(91, -10), new PointF(24, 2)
                    };
                PointF[] lower = _behavior == "flight"
                    ? new PointF[]
                    {
                        new PointF(-5, 13), new PointF(34, 58 + wing),
                        new PointF(88, 43 + wing), new PointF(28, -4)
                    }
                    : new PointF[]
                    {
                        new PointF(-3, 10), new PointF(56, 25),
                        new PointF(91, 10), new PointF(24, -2)
                    };
                graphics.FillPolygon(wingBrush, upper);
                graphics.FillPolygon(wingBrush, lower);
                graphics.DrawPolygon(wingEdge, upper);
                graphics.DrawPolygon(wingEdge, lower);
                graphics.DrawLine(wingEdge, 4, -10, 72, _behavior == "flight" ? -39 - wing : -12);
                graphics.DrawLine(wingEdge, 4, 10, 72, _behavior == "flight" ? 39 + wing : 12);
            }
            using (Pen leg = new Pen(Color.FromArgb(116, 77, 42), 2.5f))
            {
                graphics.DrawLines(leg, new PointF[] { new PointF(-14, -8), new PointF(-45, -38), new PointF(-72, -45) });
                graphics.DrawLines(leg, new PointF[] { new PointF(2, -4), new PointF(-20, -48), new PointF(-8, -65) });
                graphics.DrawLines(leg, new PointF[] { new PointF(18, -2), new PointF(45, -34), new PointF(72, -37) });
                graphics.DrawLines(leg, new PointF[] { new PointF(-14, 8), new PointF(-45, 38), new PointF(-72, 45) });
                graphics.DrawLines(leg, new PointF[] { new PointF(2, 4), new PointF(-20, 48), new PointF(-8, 65) });
                graphics.DrawLines(leg, new PointF[] { new PointF(18, 2), new PointF(45, 34), new PointF(72, 37) });
            }
            using (LinearGradientBrush abdomen = new LinearGradientBrush(
                new Rectangle(12, -21, 72, 42), Color.FromArgb(184, 126, 49),
                Color.FromArgb(73, 39, 22), 0.0f))
            using (Pen outline = new Pen(Color.FromArgb(78, 45, 28), 2.0f))
            {
                graphics.FillEllipse(abdomen, 8, -22, 82, 44);
                graphics.DrawEllipse(outline, 8, -22, 82, 44);
                int x;
                for (x = 31; x <= 72; x += 14) graphics.DrawLine(outline, x, -18, x, 18);
            }
            using (Brush thorax = new SolidBrush(Color.FromArgb(124, 82, 38)))
            using (Pen outline = new Pen(Color.FromArgb(65, 38, 23), 2.2f))
            {
                graphics.FillEllipse(thorax, -22, -28, 55, 56);
                graphics.DrawEllipse(outline, -22, -28, 55, 56);
            }
            using (Brush head = new SolidBrush(Color.FromArgb(78, 52, 33)))
            using (Brush eye = new SolidBrush(Color.FromArgb(190, 43, 39)))
            using (Pen eyeEdge = new Pen(Color.FromArgb(251, 111, 72), 1.5f))
            {
                graphics.FillEllipse(head, -53, -23, 40, 46);
                graphics.FillEllipse(eye, -55, -18, 18, 30);
                graphics.FillEllipse(eye, -31, -18, 18, 30);
                graphics.DrawEllipse(eyeEdge, -55, -18, 18, 30);
                graphics.DrawEllipse(eyeEdge, -31, -18, 18, 30);
            }
            using (Pen antenna = new Pen(Color.FromArgb(91, 62, 38), 1.8f))
            {
                graphics.DrawLine(antenna, -46, -17, -75, -38);
                graphics.DrawLine(antenna, -46, 17, -75, 38);
            }
        }

        private void DrawFirefly(Graphics graphics)
        {
            float wing = _behavior == "flight" ? (float)Math.Sin(_phase) * 10.0f : 0.0f;
            using (Brush halo = new SolidBrush(Color.FromArgb(52, 197, 255, 92)))
            {
                graphics.FillEllipse(halo, 42, -43, 91, 86);
            }
            if (_behavior == "flight")
            {
                using (Brush wingBrush = new SolidBrush(Color.FromArgb(108, 91, 221, 213)))
                using (Pen wingEdge = new Pen(Color.FromArgb(185, 161, 246, 222), 1.8f))
                {
                    PointF[] upper = new PointF[]
                    {
                        new PointF(-4, -11), new PointF(35, -57 - wing),
                        new PointF(94, -38 - wing), new PointF(23, 4)
                    };
                    PointF[] lower = new PointF[]
                    {
                        new PointF(-4, 11), new PointF(35, 57 + wing),
                        new PointF(94, 38 + wing), new PointF(23, -4)
                    };
                    graphics.FillPolygon(wingBrush, upper);
                    graphics.FillPolygon(wingBrush, lower);
                    graphics.DrawPolygon(wingEdge, upper);
                    graphics.DrawPolygon(wingEdge, lower);
                }
            }
            using (Pen leg = new Pen(Color.FromArgb(42, 79, 58), 3.0f))
            {
                graphics.DrawLines(leg, new PointF[] { new PointF(-12, -8), new PointF(-42, -38), new PointF(-72, -44) });
                graphics.DrawLines(leg, new PointF[] { new PointF(3, -5), new PointF(-18, -49), new PointF(-5, -66) });
                graphics.DrawLines(leg, new PointF[] { new PointF(20, -2), new PointF(48, -34), new PointF(75, -36) });
                graphics.DrawLines(leg, new PointF[] { new PointF(-12, 8), new PointF(-42, 38), new PointF(-72, 44) });
                graphics.DrawLines(leg, new PointF[] { new PointF(3, 5), new PointF(-18, 49), new PointF(-5, 66) });
                graphics.DrawLines(leg, new PointF[] { new PointF(20, 2), new PointF(48, 34), new PointF(75, 36) });
            }
            using (LinearGradientBrush elytra = new LinearGradientBrush(
                new Rectangle(3, -31, 92, 62), Color.FromArgb(46, 128, 72),
                Color.FromArgb(5, 31, 26), 0.0f))
            using (Pen edge = new Pen(Color.FromArgb(104, 209, 120), 2.2f))
            {
                graphics.FillPie(elytra, 0, -31, 96, 62, 180, 180);
                graphics.FillPie(elytra, 0, -31, 96, 62, 0, 180);
                graphics.DrawArc(edge, 0, -31, 96, 62, 180, 180);
                graphics.DrawArc(edge, 0, -31, 96, 62, 0, 180);
                graphics.DrawLine(edge, 2, 0, 86, 0);
            }
            using (Brush lantern = new SolidBrush(Color.FromArgb(221, 205, 255, 107)))
            using (Pen lanternEdge = new Pen(Color.FromArgb(240, 238, 255, 153), 2.0f))
            {
                graphics.FillEllipse(lantern, 72, -19, 47, 38);
                graphics.DrawEllipse(lanternEdge, 72, -19, 47, 38);
            }
            using (LinearGradientBrush shield = new LinearGradientBrush(
                new Rectangle(-25, -29, 53, 58), Color.FromArgb(255, 204, 75),
                Color.FromArgb(136, 51, 37), 90.0f))
            using (Pen edge = new Pen(Color.FromArgb(255, 225, 111), 2.2f))
            {
                graphics.FillEllipse(shield, -25, -29, 53, 58);
                graphics.DrawEllipse(edge, -25, -29, 53, 58);
            }
            using (Brush head = new SolidBrush(Color.FromArgb(5, 23, 19)))
            using (Brush eye = new SolidBrush(Color.FromArgb(226, 94, 54)))
            {
                graphics.FillEllipse(head, -57, -23, 38, 46);
                graphics.FillEllipse(eye, -55, -15, 14, 12);
                graphics.FillEllipse(eye, -55, 3, 14, 12);
            }
            using (Pen antenna = new Pen(Color.FromArgb(91, 132, 102), 1.8f))
            {
                graphics.DrawLine(antenna, -49, -16, -78, -38);
                graphics.DrawLine(antenna, -49, 16, -78, 38);
            }
        }
    }
}
