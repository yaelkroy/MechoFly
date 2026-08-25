using System;
using System.Drawing;
using System.Threading;
using System.Windows.Forms;

namespace MechoFly
{
    internal static class Program
    {
        [STAThread]
        private static int Main(string[] args)
        {
            if (args.Length > 0 && string.Equals(args[0], "--self-test", StringComparison.OrdinalIgnoreCase))
            {
                string receipt = args.Length > 1 ? args[1] : "mechofly-self-test.json";
                return SelfTest.Run(receipt);
            }

            bool created;
            using (Mutex mutex = new Mutex(true, "Local\\MechoFly.Singleton", out created))
            {
                if (!created) return 3;
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                using (MechoFlyApplicationContext context = new MechoFlyApplicationContext())
                {
                    Application.Run(context);
                }
            }
            return 0;
        }
    }

    internal sealed class MechoFlyApplicationContext : ApplicationContext, IDisposable
    {
        private readonly SimulationCoordinator _coordinator;
        private readonly FlyOverlayForm _overlay;
        private readonly NotifyIcon _tray;
        private BrainLabForm _brainLab;
        private ToolStripMenuItem _pauseItem;
        private bool _disposed;

        public MechoFlyApplicationContext()
        {
            _coordinator = new SimulationCoordinator(true);
            _overlay = new FlyOverlayForm(_coordinator);
            _overlay.Show();

            ContextMenuStrip menu = new ContextMenuStrip();
            ToolStripMenuItem lab = new ToolStripMenuItem("Open Brain Lab");
            lab.Click += delegate { ShowBrainLab(); };
            menu.Items.Add(lab);
            _pauseItem = new ToolStripMenuItem("Pause modeled activity");
            _pauseItem.Click += TogglePaused;
            menu.Items.Add(_pauseItem);
            menu.Items.Add(new ToolStripSeparator());
            ToolStripMenuItem quit = new ToolStripMenuItem("Quit MechoFly");
            quit.Click += delegate { ExitThread(); };
            menu.Items.Add(quit);

            _tray = new NotifyIcon();
            _tray.Icon = SystemIcons.Information;
            _tray.Text = "MechoFly — modeled neural companion";
            _tray.ContextMenuStrip = menu;
            _tray.DoubleClick += delegate { ShowBrainLab(); };
            _tray.Visible = true;
        }

        private void ShowBrainLab()
        {
            if (_brainLab == null || _brainLab.IsDisposed)
            {
                _brainLab = new BrainLabForm(_coordinator);
            }
            _brainLab.Show();
            _brainLab.BringToFront();
            _brainLab.Activate();
        }

        private void TogglePaused(object sender, EventArgs eventArgs)
        {
            bool paused = !_coordinator.Paused;
            _coordinator.SetPaused(paused);
            _pauseItem.Text = paused ? "Resume modeled activity" : "Pause modeled activity";
        }

        protected override void ExitThreadCore()
        {
            Dispose();
            base.ExitThreadCore();
        }

        public new void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            _tray.Visible = false;
            _tray.Dispose();
            if (_brainLab != null && !_brainLab.IsDisposed) _brainLab.Close();
            if (!_overlay.IsDisposed) _overlay.Close();
            _overlay.Dispose();
            _coordinator.Dispose();
            base.Dispose();
        }
    }
}

