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

            VisualSkin initialSkin;
            try
            {
                initialSkin = ReadSkin(args);
            }
            catch (ArgumentException)
            {
                return 64;
            }

            bool created;
            using (Mutex mutex = new Mutex(true, "Local\\MechoFly.Singleton", out created))
            {
                if (!created) return 3;
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                using (MechoFlyApplicationContext context = new MechoFlyApplicationContext(initialSkin))
                {
                    Application.Run(context);
                }
            }
            return 0;
        }

        private static VisualSkin ReadSkin(string[] args)
        {
            VisualSkin skin = SkinCatalog.Default;
            int i;
            for (i = 0; i < args.Length; i++)
            {
                if (string.Equals(args[i], "--skin", StringComparison.OrdinalIgnoreCase))
                {
                    if (i + 1 >= args.Length)
                    {
                        throw new ArgumentException("--skin requires a value.");
                    }
                    skin = SkinCatalog.ParseRequired(args[++i]);
                }
                else
                {
                    throw new ArgumentException("Unsupported command-line option: " + args[i]);
                }
            }
            return skin;
        }
    }

    internal sealed class MechoFlyApplicationContext : ApplicationContext, IDisposable
    {
        private readonly SimulationCoordinator _coordinator;
        private readonly FlyOverlayForm _overlay;
        private readonly NotifyIcon _tray;
        private BrainLabForm _brainLab;
        private ToolStripMenuItem _pauseItem;
        private ToolStripMenuItem _drosophilaSkinItem;
        private ToolStripMenuItem _fireflySkinItem;
        private bool _disposed;

        public MechoFlyApplicationContext(VisualSkin initialSkin)
        {
            _coordinator = new SimulationCoordinator(true);
            _overlay = new FlyOverlayForm(_coordinator, initialSkin);
            _overlay.Show();

            ContextMenuStrip menu = new ContextMenuStrip();
            ToolStripMenuItem lab = new ToolStripMenuItem("Open Brain Lab");
            lab.Click += delegate { ShowBrainLab(); };
            menu.Items.Add(lab);
            _pauseItem = new ToolStripMenuItem("Pause modeled activity");
            _pauseItem.Click += TogglePaused;
            menu.Items.Add(_pauseItem);
            ToolStripMenuItem skins = new ToolStripMenuItem("Skin");
            _drosophilaSkinItem = new ToolStripMenuItem("Drosophila Natural (default)");
            _fireflySkinItem = new ToolStripMenuItem("Firefly Prism");
            _drosophilaSkinItem.Click += delegate { SetSkin(VisualSkin.Drosophila); };
            _fireflySkinItem.Click += delegate { SetSkin(VisualSkin.Firefly); };
            skins.DropDownItems.Add(_drosophilaSkinItem);
            skins.DropDownItems.Add(_fireflySkinItem);
            menu.Items.Add(skins);
            SetSkin(initialSkin);
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

        private void SetSkin(VisualSkin skin)
        {
            _overlay.SetSkin(skin);
            _drosophilaSkinItem.Checked = skin == VisualSkin.Drosophila;
            _fireflySkinItem.Checked = skin == VisualSkin.Firefly;
            _drosophilaSkinItem.CheckState = _drosophilaSkinItem.Checked
                ? CheckState.Checked : CheckState.Unchecked;
            _fireflySkinItem.CheckState = _fireflySkinItem.Checked
                ? CheckState.Checked : CheckState.Unchecked;
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
