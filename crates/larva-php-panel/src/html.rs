//! HTML templates for the control panel.
//! Single-file SPA with Tailwind CSS + Alpine.js.

/// Main HTML page served at all panel routes (SPA).
pub fn panel_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LarvaPHP Panel</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script defer src="https://unpkg.com/alpinejs@3.x/dist/cdn.min.js"></script>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
    <style>
        @import url('https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css');
        :root {
            --cyber-primary: #00f2ff;
            --cyber-secondary: #7000ff;
            --cyber-accent: #00ff9f;
            --cyber-bg: #050510;
            --cyber-surface: #0a0a1a;
            --cyber-border: #1a1a2e;
            --cyber-text: #e0e0ff;
            --cyber-text-muted: #8888aa;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        html { scroll-behavior: smooth; }
        body {
            font-family: 'Inter', sans-serif;
            background: var(--cyber-bg);
            color: var(--cyber-text);
            min-h-screen: 100vh;
        }
        .font-mono { font-family: 'JetBrains Mono', monospace; }
        .bg-cyber { background: var(--cyber-bg); }
        .bg-cyber-surface { background: var(--cyber-surface); }
        .border-cyber { border-color: var(--cyber-border); }
        .text-cyber-primary { color: var(--cyber-primary); }
        .text-cyber-accent { color: var(--cyber-accent); }
        .text-cyber-secondary { color: var(--cyber-secondary); }
        .bg-cyber-primary { background: var(--cyber-primary); color: #000; }
        .bg-cyber-secondary { background: var(--cyber-secondary); }
        .bg-cyber-accent { background: var(--cyber-accent); color: #000; }
        .shadow-cyber { box-shadow: 0 0 20px rgba(0, 242, 255, 0.15); }
        .shadow-cyber-lg { box-shadow: 0 0 40px rgba(112, 0, 255, 0.2); }
        .cyber-card {
            background: var(--cyber-surface);
            border: 1px solid var(--cyber-border);
            border-radius: 12px;
            transition: all 0.3s ease;
        }
        .cyber-card:hover {
            border-color: rgba(0, 242, 255, 0.3);
            transform: translateY(-2px);
            box-shadow: 0 4px 20px rgba(0, 242, 255, 0.1);
        }
        .btn-cyber {
            background: linear-gradient(135deg, var(--cyber-primary), var(--cyber-secondary));
            color: white;
            border: none;
            border-radius: 8px;
            padding: 0.75rem 1.5rem;
            font-weight: 600;
            transition: all 0.3s ease;
            cursor: pointer;
            text-transform: uppercase;
            font-size: 0.875rem;
            letter-spacing: 0.05em;
        }
        .btn-cyber:hover {
            transform: translateY(-1px);
            box-shadow: 0 0 15px rgba(0, 242, 255, 0.4);
        }
        .btn-cyber:active { transform: translateY(0); }
        .btn-ghost {
            background: transparent;
            color: var(--cyber-text);
            border: 1px solid var(--cyber-border);
            border-radius: 8px;
            padding: 0.75rem 1.5rem;
            font-weight: 500;
            transition: all 0.3s ease;
            cursor: pointer;
        }
        .btn-ghost:hover {
            border-color: var(--cyber-primary);
            color: var(--cyber-primary);
            background: rgba(0, 242, 255, 0.05);
        }
        .input-cyber {
            background: rgba(10, 10, 26, 0.6);
            border: 1px solid var(--cyber-border);
            border-radius: 8px;
            padding: 0.75rem 1rem;
            color: var(--cyber-text);
            width: 100%;
            transition: all 0.3s ease;
        }
        .input-cyber:focus {
            outline: none;
            border-color: var(--cyber-primary);
            box-shadow: 0 0 0 2px rgba(0, 242, 255, 0.2);
        }
        .select-cyber {
            background: rgba(10, 10, 26, 0.6);
            border: 1px solid var(--cyber-border);
            border-radius: 8px;
            padding: 0.75rem 1rem;
            color: var(--cyber-text);
            width: 100%;
            cursor: pointer;
        }
        .nav-item {
            padding: 0.875rem 1.25rem;
            border-radius: 8px;
            color: var(--cyber-text-muted);
            font-weight: 500;
            transition: all 0.2s ease;
            cursor: pointer;
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }
        .nav-item:hover {
            background: rgba(0, 242, 255, 0.05);
            color: var(--cyber-primary);
        }
        .nav-item.active {
            background: linear-gradient(135deg, rgba(0, 242, 255, 0.15), rgba(112, 0, 255, 0.15));
            color: var(--cyber-primary);
            border-left: 3px solid var(--cyber-primary);
        }
        .stat-card {
            background: var(--cyber-surface);
            border: 1px solid var(--cyber-border);
            border-radius: 12px;
            padding: 1.5rem;
            position: relative;
            overflow: hidden;
        }
        .stat-card::before {
            content: '';
            position: absolute;
            top: 0; left: 0; right: 0; height: 2px;
            background: linear-gradient(90deg, var(--cyber-primary), var(--cyber-secondary));
        }
        .badge {
            display: inline-flex;
            align-items: center;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 600;
            text-transform: uppercase;
        }
        .badge-active { background: rgba(0, 255, 159, 0.15); color: var(--cyber-accent); border: 1px solid rgba(0, 255, 159, 0.3); }
        .badge-stopped { background: rgba(255, 0, 100, 0.15); color: #ff0064; border: 1px solid rgba(255, 0, 100, 0.3); }
        .badge-error { background: rgba(255, 170, 0, 0.15); color: #ffaa00; border: 1px solid rgba(255, 170, 0, 0.3); }
        .table-wrapper { overflow-x: auto; }
        table { width: 100%; border-collapse: collapse; }
        th { text-align: left; padding: 1rem; color: var(--cyber-text-muted); font-size: 0.75rem; text-transform: uppercase; font-weight: 600; border-bottom: 1px solid var(--cyber-border); }
        td { padding: 1rem; border-bottom: 1px solid var(--cyber-border); color: var(--cyber-text); font-size: 0.9rem; }
        tr:hover td { background: rgba(0, 242, 255, 0.03); }
        .progress-bar {
            width: 100%; height: 6px; background: var(--cyber-border); border-radius: 3px; overflow: hidden;
        }
        .progress-fill { height: 100%; border-radius: 3px; transition: width 0.5s ease; }
        .progress-fill-primary { background: var(--cyber-primary); }
        .progress-fill-secondary { background: var(--cyber-secondary); }
        .progress-fill-accent { background: var(--cyber-accent); }
        .modal-overlay {
            position: fixed; top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(5, 5, 16, 0.8);
            backdrop-filter: blur(8px);
            z-index: 50; display: none;
            align-items: center; justify-content: center;
        }
        .modal-overlay.active { display: flex; }
        .modal {
            background: var(--cyber-surface);
            border: 1px solid var(--cyber-border);
            border-radius: 12px;
            padding: 2rem;
            width: 90%; max-width: 500px;
            max-height: 90vh; overflow-y: auto;
            transform: scale(0.95); transition: transform 0.3s ease;
        }
        .modal-overlay.active .modal { transform: scale(1); }
        .form-group { margin-bottom: 1.25rem; }
        .form-label { display: block; margin-bottom: 0.5rem; color: var(--cyber-text-muted); font-size: 0.875rem; font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em; }
        .alert {
            padding: 1rem; border-radius: 8px; margin-bottom: 1rem; font-size: 0.9rem;
        }
        .alert-success { background: rgba(0, 255, 159, 0.1); border: 1px solid rgba(0, 255, 159, 0.3); color: var(--cyber-accent); }
        .alert-error { background: rgba(255, 0, 100, 0.1); border: 1px solid rgba(255, 0, 100, 0.3); color: #ff0064; }
        .alert-info { background: rgba(0, 242, 255, 0.1); border: 1px solid rgba(0, 242, 255, 0.3); color: var(--cyber-primary); }
        .animate-pulse-slow { animation: pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite; }
        @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }
        .sidebar { position: fixed; top: 0; left: 0; bottom: 0; width: 260px; background: var(--cyber-surface); border-right: 1px solid var(--cyber-border); z-index: 40; transform: translateX(0); transition: transform 0.3s ease; }
        @media (max-width: 768px) { .sidebar { transform: translateX(-100%); } .sidebar.mobile-open { transform: translateX(0); } }
        .main-content { margin-left: 260px; }
        @media (max-width: 768px) { .main-content { margin-left: 0; } }
        .mobile-menu-btn { display: none; }
        @media (max-width: 768px) { .mobile-menu-btn { display: block; } }
        .topbar { position: fixed; top: 0; left: 260px; right: 0; height: 64px; background: var(--cyber-surface); border-bottom: 1px solid var(--cyber-border); z-index: 30; display: flex; align-items: center; justify-content: space-between; padding: 0 1.5rem; }
        @media (max-width: 768px) { .topbar { left: 0; } }
        .loading-spinner { border: 3px solid var(--cyber-border); border-top: 3px solid var(--cyber-primary); border-radius: 50%; width: 24px; height: 24px; animation: spin 1s linear infinite; }
        @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
        .text-gradient { background: linear-gradient(135deg, var(--cyber-primary), var(--cyber-secondary)); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .glow-text { text-shadow: 0 0 10px rgba(0, 242, 255, 0.3); }
    </style>
</head>
<body x-data="{ sidebarOpen: false, alert: null, alertType: 'info' }"
      x-init="fetchStats()"
      class="min-h-screen">

    <!-- Sidebar -->
    <aside class="sidebar" :class="{ 'mobile-open': sidebarOpen }">
        <div class="p-6 flex items-center gap-3">
            <div class="w-8 h-8 bg-gradient-to-br from-cyber-primary to-cyber-secondary rounded-lg flex items-center justify-center shadow-cyber-lg">
                <i class="fa fa-rocket text-white text-sm"></i>
            </div>
            <span class="font-bold text-lg text-cyber-primary tracking-tight">Larva<span class="text-white">PHP</span></span>
        </div>
        <nav class="mt-6 px-4 space-y-1">
            <div class="text-xs font-bold text-cyber-text-muted uppercase px-4 mb-4">Navigation</div>
            <button @click="currentPage='dashboard'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'dashboard' }">
                <i class="fa fa-chart-line w-5"></i> Dashboard
            </button>
            <button @click="currentPage='websites'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'websites' }">
                <i class="fa fa-globe w-5"></i> Websites
            </button>
            <button @click="currentPage='ssl'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'ssl' }">
                <i class="fa fa-lock w-5"></i> SSL / TLS
            </button>
            <button @click="currentPage='workers'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'workers' }">
                <i class="fa fa-cogs w-5"></i> PHP Workers
            </button>
            <button @click="currentPage='files'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'files' }">
                <i class="fa fa-folder w-5"></i> File Manager
            </button>
            <button @click="currentPage='users'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'users' }">
                <i class="fa fa-users w-5"></i> Users
            </button>
            <button @click="currentPage='settings'; closeModal()" class="nav-item" :class="{ 'active': currentPage === 'settings' }">
                <i class="fa fa-sliders-h w-5"></i> Settings
            </button>
        </nav>
        <div class="absolute bottom-0 left-0 right-0 p-4 border-t border-cyber-border">
            <button @click="sidebarOpen = false" class="w-full btn-ghost text-sm">
                <i class="fa fa-power-off mr-2"></i> Logout
            </button>
        </div>
    </aside>

    <!-- Mobile overlay -->
    <div x-show="sidebarOpen" x-transition @click="sidebarOpen = false" class="fixed inset-0 bg-black/40 z-49"></div>

    <!-- Topbar -->
    <header class="topbar">
        <div class="flex items-center gap-4">
            <button @click="sidebarOpen = !sidebarOpen" class="mobile-menu-btn text-cyber-text-muted hover:text-cyber-primary">
                <i class="fa fa-bars text-xl"></i>
            </button>
            <h1 class="text-lg font-semibold text-cyber-text" x-text="pageTitle"></h1>
        </div>
        <div class="flex items-center gap-3">
            <span class="hidden sm:block w-px h-5 bg-cyber-border"></span>
            <div class="flex items-center gap-2 px-3 py-1 bg-cyber-border rounded-full text-xs text-cyber-text-muted">
                <span class="w-2 h-2 rounded-full bg-cyber-accent animate-pulse"></span>
                <span>System Online</span>
            </div>
            <button class="text-cyber-text-muted hover:text-cyber-primary transition-colors">
                <i class="fa fa-bell text-xl"></i>
            </button>
        </div>
    </header>

    <!-- Main Content -->
    <main class="main-content pt-64 pb-8">
        <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">

            <!-- Alert -->
            <div x-show="alert" x-transition class="alert" :class="`alert-${alertType}`" x-text="alert"></div>

            <!-- Dashboard -->
            <div x-show="currentPage === 'dashboard'" class="space-y-6">
                <div class="flex justify-between items-end">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">Dashboard</h2>
                        <p class="text-cyber-text-muted">Server overview and performance metrics</p>
                    </div>
                    <button @click="fetchStats()" class="btn-ghost text-sm">
                        <i class="fa fa-refresh mr-2"></i> Refresh
                    </button>
                </div>

                <!-- Stats -->
                <div class="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-6 gap-4">
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">Total Websites</div>
                        <div class="text-2xl font-bold text-cyber-primary" x-text="stats.total_websites">0</div>
                    </div>
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">Active Websites</div>
                        <div class="text-2xl font-bold text-cyber-accent" x-text="stats.active_websites">0</div>
                    </div>
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">PHP Workers</div>
                        <div class="text-2xl font-bold text-cyber-secondary" x-text="stats.total_workers">0</div>
                    </div>
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">Running</div>
                        <div class="text-2xl font-bold text-cyber-accent" x-text="stats.running_workers">0</div>
                    </div>
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">SSL Certs</div>
                        <div class="text-2xl font-bold text-cyber-primary" x-text="stats.ssl_certificates">0</div>
                    </div>
                    <div class="stat-card">
                        <div class="text-cyber-text-muted text-sm font-semibold uppercase tracking-wider mb-1">Requests Today</div>
                        <div class="text-2xl font-bold text-cyber-text" x-text="stats.requests_today">0</div>
                    </div>
                </div>

                <!-- Resource Usage -->
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    <div class="cyber-card p-6">
                        <h3 class="text-cyber-primary font-semibold mb-4">CPU Usage</h3>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm" x-text="Math.round(stats.cpu_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-cyber-text-muted font-mono" x-text="stats.cpu_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-bar"><div class="progress-fill progress-fill-primary" :style="`width: ${stats.cpu_usage}%`"></div></div>
                    </div>
                    <div class="cyber-card p-6">
                        <h3 class="text-cyber-secondary font-semibold mb-4">Memory Usage</h3>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm" x-text="Math.round(stats.memory_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-cyber-text-muted font-mono" x-text="stats.memory_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-bar"><div class="progress-fill progress-fill-secondary" :style="`width: ${stats.memory_usage}%`"></div></div>
                    </div>
                    <div class="cyber-card p-6">
                        <h3 class="text-cyber-accent font-semibold mb-4">Disk Usage</h3>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm" x-text="Math.round(stats.disk_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-cyber-text-muted font-mono" x-text="stats.disk_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-bar"><div class="progress-fill progress-fill-accent" :style="`width: ${stats.disk_usage}%`"></div></div>
                    </div>
                </div>

                <!-- Quick Actions -->
                <div class="cyber-card p-6">
                    <h3 class="text-cyber-text font-semibold text-lg mb-6">Quick Actions</h3>
                    <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
                        <button @click="currentPage='websites'; showAddWebsiteModal = true" class="btn-cyber py-4">
                            <i class="fa fa-plus mr-2"></i> Add Website
                        </button>
                        <button @click="currentPage='ssl'; showAddSslModal = true" class="btn-cyber py-4">
                            <i class="fa fa-lock mr-2"></i> Issue SSL
                        </button>
                        <button @click="fetchWorkers()" class="btn-cyber py-4">
                            <i class="fa fa-sync mr-2"></i> Sync Workers
                        </button>
                        <button @click="showAddUserModal = true" class="btn-cyber py-4">
                            <i class="fa fa-user-plus mr-2"></i> Add User
                        </button>
                    </div>
                </div>
            </div>

            <!-- Websites -->
            <div x-show="currentPage === 'websites'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">Websites</h2>
                        <p class="text-cyber-text-muted">Manage your PHP applications</p>
                    </div>
                    <button @click="showAddWebsiteModal = true" class="btn-cyber">
                        <i class="fa fa-plus mr-2"></i> Add Website
                    </button>
                </div>

                <div class="cyber-card p-0 overflow-hidden">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th>Domain</th>
                                    <th>Document Root</th>
                                    <th>PHP Version</th>
                                    <th>SSL</th>
                                    <th>Workers</th>
                                    <th>Status</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="site in websites" :key="site.id">
                                    <td class="font-mono text-sm" x-text="site.domain">example.com</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="site.document_root">/var/www/html</td>
                                    <td class="font-mono text-xs" x-text="site.php_version">8.3</td>
                                    <td><span class="badge" :class="site.ssl_enabled ? 'badge-active' : 'badge-stopped'" x-text="site.ssl_enabled ? 'Active' : 'Inactive'">Active</span></td>
                                    <td class="font-mono text-sm" x-text="site.worker_count">1</td>
                                    <td><span class="badge badge-active" x-text="site.is_active ? 'Online' : 'Offline'">Online</span></td>
                                    <td class="flex gap-2">
                                        <button @click="editWebsite(site)" class="btn-ghost text-xs py-1.5 px-3 text-cyber-primary hover:text-cyber-primary border-cyber-primary/30 hover:border-cyber-primary/50">
                                            <i class="fa fa-edit"></i>
                                        </button>
                                        <button @click="deleteWebsite(site.id)" class="btn-ghost text-xs py-1.5 px-3 text-red-400 hover:text-red-400 border-red-400/30 hover:border-red-400/50">
                                            <i class="fa fa-trash"></i>
                                        </button>
                                    </td>
                                </tr>
                                <tr x-show="websites.length === 0">
                                    <td colspan="7" class="text-center py-8 text-cyber-text-muted">No websites yet. Add your first website.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- SSL / TLS -->
            <div x-show="currentPage === 'ssl'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">SSL / TLS Certificates</h2>
                        <p class="text-cyber-text-muted">Manage Let's Encrypt certificates via ACME</p>
                    </div>
                    <button @click="showAddSslModal = true" class="btn-cyber">
                        <i class="fa fa-plus mr-2"></i> Issue Certificate
                    </button>
                </div>

                <div class="cyber-card p-0 overflow-hidden">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th>Domain</th>
                                    <th>Issuer</th>
                                    <th>Expires</th>
                                    <th>Auto Renew</th>
                                    <th>Status</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="cert in sslCerts" :key="cert.id">
                                    <td class="font-mono text-sm" x-text="cert.domain">example.com</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="cert.issuer">letsencrypt</td>
                                    <td class="font-mono text-xs" x-text="cert.expires_at">--</td>
                                    <td><span class="badge" :class="cert.auto_renew ? 'badge-active' : 'badge-stopped'" x-text="cert.auto_renew ? 'On' : 'Off'">On</span></td>
                                    <td><span class="badge badge-active">Valid</span></td>
                                    <td class="flex gap-2">
                                        <button @click="renewCert(cert.id)" class="btn-ghost text-xs py-1.5 px-3 text-cyber-accent hover:text-cyber-accent border-cyber-accent/30 hover:border-cyber-accent/50">
                                            <i class="fa fa-sync-alt"></i>
                                        </button>
                                        <button @click="deleteCert(cert.id)" class="btn-ghost text-xs py-1.5 px-3 text-red-400 hover:text-red-400 border-red-400/30 hover:border-red-400/50">
                                            <i class="fa fa-trash"></i>
                                        </button>
                                    </td>
                                </tr>
                                <tr x-show="sslCerts.length === 0">
                                    <td colspan="6" class="text-center py-8 text-cyber-text-muted">No SSL certificates yet.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- PHP Workers -->
            <div x-show="currentPage === 'workers'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">PHP Workers</h2>
                        <p class="text-cyber-text-muted">Manage embedded PHP worker processes</p>
                    </div>
                    <button @click="fetchWorkers()" class="btn-cyber">
                        <i class="fa fa-sync mr-2"></i> Refresh
                    </button>
                </div>

                <div class="cyber-card p-0 overflow-hidden">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th>Name</th>
                                    <th>PID</th>
                                    <th>Status</th>
                                    <th>Requests</th>
                                    <th>Uptime</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="worker in workers" :key="worker.id">
                                    <td class="font-mono text-sm" x-text="worker.name">worker-1</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="worker.pid ?? '--'">--</td>
                                    <td><span class="badge" :class="worker.status === 'running' ? 'badge-active' : 'badge-stopped'" x-text="worker.status">running</span></td>
                                    <td class="font-mono text-sm" x-text="worker.requests">0</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="worker.uptime ?? '--'">--</td>
                                    <td class="flex gap-2">
                                        <button @click="restartWorker(worker.id)" class="btn-ghost text-xs py-1.5 px-3 text-cyber-primary hover:text-cyber-primary border-cyber-primary/30 hover:border-cyber-primary/50">
                                            <i class="fa fa-redo"></i>
                                        </button>
                                        <button @click="stopWorker(worker.id)" class="btn-ghost text-xs py-1.5 px-3 text-red-400 hover:text-red-400 border-red-400/30 hover:border-red-400/50">
                                            <i class="fa fa-stop"></i>
                                        </button>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- File Manager -->
            <div x-show="currentPage === 'files'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">File Manager</h2>
                        <p class="text-cyber-text-muted">Browse and edit files on the server</p>
                    </div>
                </div>
                <div class="cyber-card p-6">
                    <div class="flex gap-2 mb-4">
                        <input type="text" class="input-cyber flex-1" placeholder="Current directory" x-model="fileDir">
                        <button @click="listFiles()" class="btn-ghost px-4">
                            <i class="fa fa-search"></i>
                        </button>
                    </div>
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr><th>Name</th><th>Type</th><th>Size</th><th>Modified</th><th>Actions</th></tr>
                            </thead>
                            <tbody>
                                <tr x-for="file in files" :key="file.name">
                                    <td class="font-mono text-sm" x-text="file.name">index.php</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="file.type">file</td>
                                    <td class="font-mono text-xs" x-text="file.size">1.2 KB</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="file.modified">--</td>
                                    <td class="flex gap-2">
                                        <button @click="editFile(file.path)" class="btn-ghost text-xs py-1.5 px-3 text-cyber-primary hover:text-cyber-primary border-cyber-primary/30 hover:border-cyber-primary/50">
                                            <i class="fa fa-edit"></i>
                                        </button>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Users -->
            <div x-show="currentPage === 'users'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-cyber-text">Users</h2>
                        <p class="text-cyber-text-muted">Manage panel administrators</p>
                    </div>
                    <button @click="showAddUserModal = true" class="btn-cyber">
                        <i class="fa fa-plus mr-2"></i> Add User
                    </button>
                </div>
                <div class="cyber-card p-0 overflow-hidden">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr><th>Username</th><th>Role</th><th>Email</th><th>Status</th><th>Actions</th></tr>
                            </thead>
                            <tbody>
                                <tr x-for="user in users" :key="user.id">
                                    <td class="font-mono text-sm" x-text="user.username">admin</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="user.role">admin</td>
                                    <td class="font-mono text-xs text-cyber-text-muted" x-text="user.email ?? '—'">—</td>
                                    <td><span class="badge badge-active" x-text="user.is_active ? 'Active' : 'Inactive'">Active</span></td>
                                    <td class="flex gap-2"></td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Settings -->
            <div x-show="currentPage === 'settings'" class="space-y-6">
                <h2 class="text-2xl font-bold text-cyber-text">Settings</h2>
                <div class="cyber-card p-6">
                    <h3 class="text-cyber-text font-semibold text-lg mb-6">Server Configuration</h3>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div class="form-group">
                            <label class="form-label">Listen Address</label>
                            <input type="text" class="input-cyber" value="0.0.0.0:8000">
                        </div>
                        <div class="form-group">
                            <label class="form-label">PHP Version</label>
                            <select class="select-cyber">
                                <option>PHP 8.3</option>
                                <option>PHP 8.2</option>
                                <option>PHP 8.1</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label class="form-label">Worker Count</label>
                            <input type="number" class="input-cyber" value="4">
                        </div>
                        <div class="form-group">
                            <label class="form-label">Max Body Size</label>
                            <input type="text" class="input-cyber" value="10 MB">
                        </div>
                    </div>
                    <button class="btn-cyber mt-6">Save Settings</button>
                </div>
            </div>

        </div>
    </main>

    <!-- Add Website Modal -->
    <div x-show="showAddWebsiteModal" class="modal-overlay" x-transition @click="showAddWebsiteModal = false">
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-cyber-text mb-6">Add Website</h3>
            <form @submit.prevent="addWebsite">
                <div class="form-group">
                    <label class="form-label">Website Name</label>
                    <input type="text" class="input-cyber" x-model="newWebsite.name" placeholder="My Website" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domain</label>
                    <input type="text" class="input-cyber" x-model="newWebsite.domain" placeholder="example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Document Root</label>
                    <input type="text" class="input-cyber" x-model="newWebsite.document_root" placeholder="/var/www/example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">PHP Version</label>
                    <select class="select-cyber" x-model="newWebsite.php_version">
                        <option>8.3</option><option>8.2</option><option>8.1</option>
                    </select>
                </div>
                <div class="form-group">
                    <label class="form-label">Worker Count</label>
                    <input type="number" class="input-cyber" x-model="newWebsite.worker_count" value="1" min="1">
                </div>
                <div class="form-group flex items-center gap-2">
                    <input type="checkbox" class="rounded text-cyber-primary focus:ring-cyber-primary" x-model="newWebsite.ssl_enabled">
                    <label class="text-cyber-text-muted text-sm">Enable SSL (Let's Encrypt)</label>
                </div>
                <div class="form-group" x-show="newWebsite.ssl_enabled">
                    <label class="form-label">ACME Email</label>
                    <input type="email" class="input-cyber" x-model="newWebsite.acme_email" placeholder="admin@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddWebsiteModal = false" class="btn-ghost px-4">Cancel</button>
                    <button type="submit" class="btn-cyber px-6">Add Website</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add SSL Modal -->
    <div x-show="showAddSslModal" class="modal-overlay" x-transition @click="showAddSslModal = false">
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-cyber-text mb-6">Issue SSL Certificate</h3>
            <form @submit.prevent="issueSsl">
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-cyber" x-model="sslForm.email" placeholder="admin@example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domains (comma separated)</label>
                    <input type="text" class="input-cyber" x-model="sslForm.domains" placeholder="example.com, www.example.com" required>
                </div>
                <div class="text-xs text-cyber-text-muted mt-2">Uses HTTP-01 challenge. Port 80 must be publicly accessible.</div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddSslModal = false" class="btn-ghost px-4">Cancel</button>
                    <button type="submit" class="btn-cyber px-6">Issue Certificate</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add User Modal -->
    <div x-show="showAddUserModal" class="modal-overlay" x-transition @click="showAddUserModal = false">
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-cyber-text mb-6">Add User</h3>
            <form @submit.prevent="addUser">
                <div class="form-group">
                    <label class="form-label">Username</label>
                    <input type="text" class="input-cyber" x-model="newUser.username" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Password</label>
                    <input type="password" class="input-cyber" x-model="newUser.password" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Role</label>
                    <select class="select-cyber" x-model="newUser.role">
                        <option>admin</option><option>viewer</option>
                    </select>
                </div>
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-cyber" x-model="newUser.email" placeholder="user@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddUserModal = false" class="btn-ghost px-4">Cancel</button>
                    <button type="submit" class="btn-cyber px-6">Add User</button>
                </div>
            </form>
        </div>
    </div>

    <script>
        // Alpine.js data
        function init() {
            return {
                currentPage: 'dashboard',
                sidebarOpen: false,
                alert: null,
                alertType: 'info',
                stats: { total_websites: 0, active_websites: 0, total_workers: 0, running_workers: 0, ssl_certificates: 0, requests_today: 0, cpu_usage: 0, memory_usage: 0, disk_usage: 0 },
                websites: [],
                sslCerts: [],
                workers: [],
                files: [],
                fileDir: '/',
                showAddWebsiteModal: false,
                showAddSslModal: false,
                showAddUserModal: false,
                newWebsite: { name: '', domain: '', document_root: '', php_version: '8.3', ssl_enabled: false, acme_email: '', acme_domains: '', worker_count: 1 },
                sslForm: { email: '', domains: '' },
                newUser: { username: '', password: '', role: 'admin', email: '' },
                pageTitle: 'Dashboard',
            };
        }

        // Page titles
        const titles = {
            dashboard: 'Dashboard',
            websites: 'Websites',
            ssl: 'SSL / TLS Certificates',
            workers: 'PHP Workers',
            files: 'File Manager',
            users: 'Users',
            settings: 'Settings'
        };

        // Initialize Alpine
        document.addEventListener('alpine:init', () => {
            Alpine.data('app', init);
        });

        // Global helpers
        async function api(path, method = 'GET', body = null) {
            const opts = { method, headers: { 'Content-Type': 'application/json' } };
            if (body) opts.body = JSON.stringify(body);
            const res = await fetch('/api' + path, opts);
            const data = await res.json();
            if (!res.ok) throw new Error(data.error || 'Request failed');
            return data;
        }

        function showAlert(msg, type = 'info') {
            const app = Alpine.store('app') || document.querySelector('[x-data]');
            // Use direct Alpine reference
            const component = document.querySelector('[x-data]');
            if (component && component.__x) {
                component.__x.alert = msg;
                component.__x.alertType = type;
                setTimeout(() => { component.__x.alert = null; }, 5000);
            }
        }

        async function fetchStats() {
            try {
                const data = await api('/stats');
                const comp = document.querySelector('[x-data]');
                if (comp && comp.__x) comp.__x.stats = data.data || data;
            } catch(e) { console.warn(e); }
        }

        async function fetchWebsites() {
            try {
                const data = await api('/websites');
                const comp = document.querySelector('[x-data]');
                if (comp && comp.__x) comp.__x.websites = data.data || [];
            } catch(e) { console.warn(e); }
        }

        async function fetchSsl() {
            try {
                const data = await api('/ssl');
                const comp = document.querySelector('[x-data]');
                if (comp && comp.__x) comp.__x.sslCerts = data.data || [];
            } catch(e) { console.warn(e); }
        }

        async function fetchWorkers() {
            try {
                const data = await api('/workers');
                const comp = document.querySelector('[x-data]');
                if (comp && comp.__x) comp.__x.workers = data.data || [];
            } catch(e) { console.warn(e); }
        }

        async function addWebsite() {
            const comp = document.querySelector('[x-data]');
            if (!comp || !comp.__x) return;
            try {
                await api('/websites', 'POST', comp.__x.newWebsite);
                showAlert('Website added successfully', 'success');
                comp.__x.showAddWebsiteModal = false;
                comp.__x.newWebsite = { name: '', domain: '', document_root: '', php_version: '8.3', ssl_enabled: false, acme_email: '', acme_domains: '', worker_count: 1 };
                fetchWebsites();
                fetchStats();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function issueSsl() {
            const comp = document.querySelector('[x-data]');
            if (!comp || !comp.__x) return;
            try {
                const domains = comp.__x.sslForm.domains.split(',').map(d => d.trim()).filter(Boolean);
                await api('/ssl/issue', 'POST', { email: comp.__x.sslForm.email, domains });
                showAlert('SSL certificate issued', 'success');
                comp.__x.showAddSslModal = false;
                fetchSsl();
                fetchStats();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function addUser() {
            const comp = document.querySelector('[x-data]');
            if (!comp || !comp.__x) return;
            try {
                await api('/users', 'POST', comp.__x.newUser);
                showAlert('User added successfully', 'success');
                comp.__x.showAddUserModal = false;
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function deleteWebsite(id) {
            if(!confirm('Delete this website?')) return;
            try {
                await api('/websites/' + id, 'DELETE');
                showAlert('Website deleted', 'success');
                fetchWebsites();
                fetchStats();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function deleteCert(id) {
            if(!confirm('Delete this certificate?')) return;
            try {
                await api('/ssl/' + id, 'DELETE');
                showAlert('Certificate deleted', 'success');
                fetchSsl();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function listFiles() {
            const comp = document.querySelector('[x-data]');
            if (!comp || !comp.__x) return;
            try {
                const data = await api('/files?dir=' + encodeURIComponent(comp.__x.fileDir));
                comp.__x.files = data.data || [];
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function editFile(path) {
            try {
                const data = await api('/files?path=' + encodeURIComponent(path));
                showAlert(data.data?.content || 'File loaded', 'info');
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function restartWorker(id) {
            try {
                await api('/workers/' + id + '/restart', 'POST');
                showAlert('Worker restarted', 'success');
                fetchWorkers();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        async function stopWorker(id) {
            try {
                await api('/workers/' + id + '/stop', 'POST');
                showAlert('Worker stopped', 'success');
                fetchWorkers();
            } catch(e) { showAlert(e.message, 'error'); }
        }

        function editWebsite(site) {
            const comp = document.querySelector('[x-data]');
            if (comp && comp.__x) {
                comp.__x.newWebsite = { ...site };
                comp.__x.showAddWebsiteModal = true;
            }
        }

        function closeModal() {
            const comp = document.querySelector('[x-data]');
            if (comp && comp.__x) {
                comp.__x.showAddWebsiteModal = false;
                comp.__x.showAddSslModal = false;
                comp.__x.showAddUserModal = false;
                comp.__x.sidebarOpen = false;
            }
        }

        // Watch currentPage changes to update title
        const observerTarget = document.querySelector('[x-data]');
        if (observerTarget) {
            const origInit = observerTarget.__x?.init;
            // Simple polling fallback for title update
            setInterval(() => {
                const comp = document.querySelector('[x-data]');
                if (comp && comp.__x && comp.__x.currentPage && titles[comp.__x.currentPage]) {
                    const h1 = document.querySelector('main h1');
                    if (h1) h1.textContent = titles[comp.__x.currentPage];
                }
            }, 100);
        }

        // Initial data load
        setTimeout(() => { fetchWebsites(); fetchSsl(); fetchWorkers(); }, 500);
    </script>
</body>
</html>"#
    .to_string()
}
