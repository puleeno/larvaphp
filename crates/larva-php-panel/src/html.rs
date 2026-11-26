//! HTML templates for the control panel.
//! Single-file SPA with Tailwind CSS + Alpine.js.
//! Ant Design inspired: clean, professional, blue/gray palette.

/// Main HTML page served at all panel routes (SPA).
pub fn panel_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LarvaPHP Panel</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script defer src="https://unpkg.com/alpinejs@3.x/dist/cdn.min.js"></script>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>
        :root {
            --ad-primary: #1677ff;
            --ad-primary-hover: #4096ff;
            --ad-primary-active: #096dd9;
            --ad-bg: #ffffff;
            --ad-bg-secondary: #f5f7f9;
            --ad-text: #000000;
            --ad-text-secondary: #595959;
            --ad-text-tertiary: #999999;
            --ad-border: #d9d9d9;
            --ad-border-hover: #4096ff;
            --ad-success: #52c41a;
            --ad-warning: #faad14;
            --ad-error: #f5222d;
            --ad-radius: 6px;
            --ad-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
            --ad-font: 'Inter', sans-serif;
            --ad-mono: 'JetBrains Mono', monospace;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        html { scroll-behavior: smooth; }
        body {
            font-family: var(--ad-font);
            background: var(--ad-bg-secondary);
            color: var(--ad-text);
            min-h-screen: 100vh;
            line-height: 1.5715;
        }
        .font-mono { font-family: var(--ad-mono); }
        .ad-sidebar { width: 240px; flex-shrink: 0; }
        .ad-main { margin-left: 240px; }
        @media (max-width: 768px) {
            .ad-sidebar { position: fixed; top: 0; left: 0; bottom: 0; z-index: 40; transform: translateX(-100%); transition: transform 0.3s ease; }
            .ad-sidebar.mobile-open { transform: translateX(0); }
            .ad-main { margin-left: 0; }
            .mobile-menu-btn { display: block; }
        }
        .mobile-menu-btn { display: none; }
        .ad-topbar { position: fixed; top: 0; left: 240px; right: 0; height: 64px; z-index: 30; }
        @media (max-width: 768px) { .ad-topbar { left: 0; } }
        .ad-mobile-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.08); z-index: 39; display: none; }
        .ad-mobile-overlay.mobile-open { display: block; }
        .ad-logo { height: 32px; width: 32px; border-radius: 6px; background: linear-gradient(135deg, #1677ff, #4096ff); display: flex; align-items: center; justify-content: center; color: #fff; font-size: 14px; }
        .nav-item-active { background: #e6f4ff; color: #1677ff; border-left: 3px solid #1677ff; }
        .nav-item-active .fa { color: #1677ff; }
        .page-title { font-size: 20px; font-weight: 600; color: #000; }
        .page-subtitle { font-size: 14px; color: #999; margin-top: 4px; }
        .stat-card-value { font-size: 24px; font-weight: 600; color: #000; }
        .stat-card-label { font-size: 12px; font-weight: 600; color: #999; text-transform: uppercase; letter-spacing: 0.05em; margin-top: 6px; }
        .ad-card-title { font-size: 14px; font-weight: 600; color: #000; margin-bottom: 16px; }
        .progress-h { height: 8px; border-radius: 4px; background: #f0f0f0; overflow: hidden; }
        .progress-fill { height: 100%; border-radius: 4px; transition: width 0.3s ease; }
        .badge { display: inline-flex; align-items: center; padding: 2px 10px; border-radius: 6px; font-size: 11px; font-weight: 600; text-transform: uppercase; }
        .badge-default { background: #f0f0f0; color: #666; }
        .badge-primary { background: #e6f4ff; color: #1677ff; }
        .badge-success { background: #f6ffed; color: #52c41a; border: 1px solid #b7eb8f; }
        .badge-warning { background: #fff7e6; color: #faad14; border: 1px solid #ffd591; }
        .badge-error { background: #fff1f0; color: #f5222d; border: 1px solid #ffa39e; }
        .alert { padding: 10px 16px; border-radius: 6px; font-size: 14px; margin-bottom: 16px; }
        .alert-info { background: #e6f4ff; border: 1px solid #91caff; color: #1677ff; }
        .alert-success { background: #f6ffed; border: 1px solid #b7eb8f; color: #52c41a; }
        .alert-error { background: #fff1f0; border: 1px solid #ffa39e; color: #f5222d; }
        .table-wrapper { overflow-x: auto; }
        table { width: 100%; border-collapse: collapse; }
        th { text-align: left; padding: 12px 16px; font-size: 12px; font-weight: 600; color: #999; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #f0f0f0; }
        td { padding: 12px 16px; border-bottom: 1px solid #f0f0f0; color: #000; font-size: 14px; }
        tr:hover td { background: #fafafa; }
        .btn-primary { background: #1677ff; color: #fff; border: none; border-radius: 6px; padding: 8px 24px; font-weight: 500; font-size: 14px; cursor: pointer; transition: background 0.2s; }
        .btn-primary:hover { background: #4096ff; }
        .btn-default { background: #fff; color: #000; border: 1px solid #d9d9d9; border-radius: 6px; padding: 8px 24px; font-weight: 500; font-size: 14px; cursor: pointer; transition: all 0.2s; }
        .btn-default:hover { border-color: #4096ff; color: #1677ff; background: #f6f8ff; }
        .btn-text { background: none; border: none; color: #1677ff; font-size: 14px; cursor: pointer; padding: 4px 8px; border-radius: 4px; }
        .btn-text:hover { background: #e6f4ff; }
        .btn-danger-text { color: #f5222d; }
        .btn-danger-text:hover { background: #fff1f0; }
        .input-default { border: 1px solid #d9d9d9; border-radius: 6px; padding: 8px 12px; font-size: 14px; width: 100%; background: #fff; color: #000; transition: border 0.2s, box-shadow 0.2s; outline: none; }
        .input-default:focus { border-color: #4096ff; box-shadow: 0 0 0 2px rgba(22, 119, 255, 0.1); }
        .select-default { border: 1px solid #d9d9d9; border-radius: 6px; padding: 8px 12px; font-size: 14px; width: 100%; background: #fff; color: #000; cursor: pointer; outline: none; }
        .select-default:focus { border-color: #4096ff; box-shadow: 0 0 0 2px rgba(22, 119, 255, 0.1); }
        .form-label { display: block; margin-bottom: 8px; font-size: 14px; font-weight: 500; color: #000; }
        .form-group { margin-bottom: 24px; }
        .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.24); z-index: 50; display: none; align-items: center; justify-content: center; backdrop-filter: blur(2px); }
        .modal-overlay.active { display: flex; }
        .modal { background: #fff; border: 1px solid #d9d9d9; border-radius: 8px; padding: 24px; width: 90%; max-width: 480px; max-height: 90vh; overflow-y: auto; box-shadow: 0 4px 12px rgba(0,0,0,0.15); transform: scale(0.98); opacity: 0; transition: transform 0.2s, opacity 0.2s; }
        .modal-overlay.active .modal { transform: scale(1); opacity: 1; }
        .modal-close-btn { position: absolute; right: 16px; top: 16px; background: none; border: none; color: #999; cursor: pointer; font-size: 18px; }
        .modal-close-btn:hover { color: #000; }
        .animate-spin { animation: adspin 1s linear infinite; }
        @keyframes adspin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
        .text-gradient { background: linear-gradient(135deg, #1677ff, #4096ff); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .empty-state { text-align: center; padding: 48px 24px; color: #999; }
        .empty-state i { font-size: 48px; margin-bottom: 16px; opacity: 0.4; }
    </style>
</head>
<body x-data="{ currentPage: 'dashboard', sidebarOpen: false, alert: null, alertType: 'info', stats: { total_websites: 0, active_websites: 0, total_workers: 0, running_workers: 0, ssl_certificates: 0, requests_today: 0, cpu_usage: 0, memory_usage: 0, disk_usage: 0 }, websites: [], sslCerts: [], workers: [], files: [], fileDir: '/', showAddWebsiteModal: false, showAddSslModal: false, showAddUserModal: false, newWebsite: { name: '', domain: '', document_root: '', php_version: '8.3', ssl_enabled: false, acme_email: '', acme_domains: '', worker_count: 1 }, sslForm: { email: '', domains: '' }, newUser: { username: '', password: '', role: 'admin', email: '' }, pageTitle: 'Dashboard', installerServices: [], installerBusy: false, installerMsg: '', installerOk: true, loadInstaller() { this.installerBusy = true; this.installerMsg = ''; fetch('/api/installer/services').then(r=>r.json()).then(d=>{ this.installerServices = d.data || []; this.installerBusy = false; }).catch(e=>{ this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; }); }, installService(id) { this.installerBusy = true; this.installerMsg = ''; fetch('/api/installer/install', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ service: id }) }).then(r=>r.json()).then(d=>{ this.installerMsg = d.success ? 'Installed ' + id : (d.error || 'Failed'); this.installerOk = d.success; this.installerBusy = false; this.loadInstaller(); }).catch(e=>{ this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; }); }, uninstallService(id) { this.installerBusy = true; this.installerMsg = ''; fetch('/api/installer/uninstall', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ service: id }) }).then(r=>r.json()).then(d=>{ this.installerMsg = d.success ? 'Uninstalled ' + id : (d.error || 'Failed'); this.installerOk = d.success; this.installerBusy = false; this.loadInstaller(); }).catch(e=>{ this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; }); } }" x-init="fetchStats()" class="min-h-screen">

    <!-- Mobile overlay -->
    <div x-show="sidebarOpen" x-transition @click="sidebarOpen = false" class="ad-mobile-overlay"></div>

    <!-- Sidebar -->
    <aside class="ad-sidebar bg-white border-r border-gray-200 shadow-sm" :class="{ 'mobile-open': sidebarOpen }">
        <div class="px-5 py-4 flex items-center gap-3">
            <div class="ad-logo text-xs font-bold text-white leading-none flex items-center justify-center"><i class="fa fa-rocket"></i></div>
            <span class="font-bold text-lg text-gray-900 tracking-tight">Larva<span class="text-blue-600">PHP</span></span>
        </div>
        <nav class="mt-5 px-3 space-y-0.5">
            <div class="text-xs font-bold text-gray-400 uppercase px-3 mb-3">Navigation</div>
            <button @click="currentPage='dashboard'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'dashboard' }">
                <i class="fa fa-chart-line w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">Dashboard</span>
            </button>
            <button @click="currentPage='websites'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'websites' }">
                <i class="fa fa-globe w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">Websites</span>
            </button>
            <button @click="currentPage='ssl'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'ssl' }">
                <i class="fa fa-lock w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">SSL / TLS</span>
            </button>
            <button @click="currentPage='workers'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'workers' }">
                <i class="fa fa-cogs w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">PHP Workers</span>
            </button>
            <button @click="currentPage='files'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'files' }">
                <i class="fa fa-folder w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">File Manager</span>
            </button>
            <button @click="currentPage='installer'; sidebarOpen=false; loadInstaller()" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'installer' }">
                <i class="fa fa-download w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">Installer</span>
            </button>
            <button @click="currentPage='users'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'users' }">
                <i class="fa fa-users w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">Users</span>
            </button>
            <button @click="currentPage='settings'; sidebarOpen=false" class="w-full flex items-center gap-3 px-3 py-2.5 text-sm rounded-l-lg transition-colors" :class="{ 'nav-item-active': currentPage === 'settings' }">
                <i class="fa fa-sliders-h w-5 text-gray-400"></i> <span class="text-gray-700 font-medium">Settings</span>
            </button>
        </nav>
        <div class="absolute bottom-0 left-0 right-0 p-4 border-t border-gray-100">
            <button @click="sidebarOpen=false" class="w-full btn-text text-sm text-gray-600 hover:text-red-600 hover:bg-red-50 flex items-center gap-2">
                <i class="fa fa-power-off"></i> Logout
            </button>
        </div>
    </aside>

    <!-- Topbar -->
    <header class="ad-topbar bg-white border-b border-gray-200 z-30 flex items-center justify-between px-5 shadow-sm">
        <div class="flex items-center gap-3">
            <button @click="sidebarOpen = !sidebarOpen" class="mobile-menu-btn text-gray-500 hover:text-gray-700">
                <i class="fa fa-bars text-xl"></i>
            </button>
            <div>
                <h1 class="page-title" x-text="pageTitle">Dashboard</h1>
                <p class="page-subtitle" x-show="currentPage === 'dashboard'">Server overview and performance metrics</p>
            </div>
        </div>
        <div class="flex items-center gap-3">
            <button class="text-gray-500 hover:text-gray-700 transition-colors relative">
                <i class="fa fa-bell text-xl"></i>
                <span x-show="false" class="absolute top-0 right-0 w-2 h-2 bg-red-500 rounded-full"></span>
            </button>
            <div class="flex items-center gap-2 px-3 py-1 bg-gray-100 rounded-full text-xs text-gray-500 border border-gray-200">
                <span class="w-2 h-2 rounded-full bg-green-500 animate-pulse"></span>
                <span class="font-medium text-gray-700">Online</span>
            </div>
        </div>
    </header>

    <!-- Main Content -->
    <main class="ad-main pt-20 pb-10">
        <div class="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8">

            <!-- Alert -->
            <div x-show="alert" x-transition class="alert" :class="`alert-${alertType}`" x-text="alert"></div>

            <!-- Dashboard -->
            <div x-show="currentPage === 'dashboard'" class="space-y-6">
                <div class="flex justify-between items-end">
                    <div>
                        <h2 class="text-2xl font-bold text-gray-900">Dashboard</h2>
                        <p class="text-gray-500">Server overview and performance metrics</p>
                    </div>
                    <button @click="fetchStats()" class="btn-default text-sm">
                        <i class="fa fa-refresh mr-2"></i> Refresh
                    </button>
                </div>

                <!-- Stats -->
                <div class="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-6 gap-4">
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">Total Websites</div>
                        <div class="stat-card-value text-blue-600" x-text="stats.total_websites">0</div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">Active Websites</div>
                        <div class="stat-card-value text-green-600" x-text="stats.active_websites">0</div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">PHP Workers</div>
                        <div class="stat-card-value text-indigo-600" x-text="stats.total_workers">0</div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">Running</div>
                        <div class="stat-card-value text-green-600" x-text="stats.running_workers">0</div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">SSL Certs</div>
                        <div class="stat-card-value text-blue-600" x-text="stats.ssl_certificates">0</div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="stat-card-label">Requests Today</div>
                        <div class="stat-card-value text-gray-900" x-text="stats.requests_today">0</div>
                    </div>
                </div>

                <!-- Resource Usage -->
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="ad-card-title flex items-center gap-2"><i class="fa fa-microchip text-blue-600"></i> CPU Usage</div>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm font-medium" x-text="Math.round(stats.cpu_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-gray-500 font-mono" x-text="stats.cpu_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-h"><div class="progress-fill bg-blue-600" :style="`width: ${stats.cpu_usage}%`"></div></div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="ad-card-title flex items-center gap-2"><i class="fa fa-memory text-indigo-600"></i> Memory Usage</div>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm font-medium" x-text="Math.round(stats.memory_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-gray-500 font-mono" x-text="stats.memory_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-h"><div class="progress-fill bg-indigo-600" :style="`width: ${stats.memory_usage}%`"></div></div>
                    </div>
                    <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                        <div class="ad-card-title flex items-center gap-2"><i class="fa fa-hdd text-green-600"></i> Disk Usage</div>
                        <div class="flex items-center gap-3 mb-3">
                            <span class="text-sm font-medium" x-text="Math.round(stats.disk_usage) + '%'">0%</span>
                            <span class="ml-auto text-xs text-gray-500 font-mono" x-text="stats.disk_usage.toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="progress-h"><div class="progress-fill bg-green-600" :style="`width: ${stats.disk_usage}%`"></div></div>
                    </div>
                </div>

                <!-- Quick Actions -->
                <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                    <h3 class="text-gray-900 font-semibold text-lg mb-6">Quick Actions</h3>
                    <div class="grid grid-cols-1 md:grid-cols-4 gap-3">
                        <button @click="currentPage='websites'; showAddWebsiteModal = true" class="btn-primary py-3 text-sm flex items-center justify-center gap-2">
                            <i class="fa fa-plus"></i> Add Website
                        </button>
                        <button @click="currentPage='ssl'; showAddSslModal = true" class="btn-primary py-3 text-sm flex items-center justify-center gap-2">
                            <i class="fa fa-lock"></i> Issue SSL
                        </button>
                        <button @click="fetchWorkers()" class="btn-default py-3 text-sm flex items-center justify-center gap-2">
                            <i class="fa fa-sync"></i> Sync Workers
                        </button>
                        <button @click="showAddUserModal = true" class="btn-default py-3 text-sm flex items-center justify-center gap-2">
                            <i class="fa fa-user-plus"></i> Add User
                        </button>
                    </div>
                </div>
            </div>

            <!-- Websites -->
            <div x-show="currentPage === 'websites'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-gray-900">Websites</h2>
                        <p class="text-gray-500">Manage your PHP applications</p>
                    </div>
                    <button @click="showAddWebsiteModal = true" class="btn-primary">
                        <i class="fa fa-plus mr-2"></i> Add Website
                    </button>
                </div>

                <div class="bg-white border border-gray-200 rounded-lg overflow-hidden shadow-sm">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th class="w-1/4">Domain</th>
                                    <th>Document Root</th>
                                    <th class="w-1/6">PHP Version</th>
                                    <th class="w-1/6">SSL</th>
                                    <th class="w-1/6">Workers</th>
                                    <th class="w-1/6">Status</th>
                                    <th class="w-1/5">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="site in websites" :key="site.id">
                                    <td class="font-mono text-sm text-gray-900" x-text="site.domain">example.com</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="site.document_root">/var/www/html</td>
                                    <td class="font-mono text-xs" x-text="site.php_version">8.3</td>
                                    <td><span class="badge" :class="site.ssl_enabled ? 'badge-success' : 'badge-default'" x-text="site.ssl_enabled ? 'Active' : 'Inactive'">Active</span></td>
                                    <td class="font-mono text-sm" x-text="site.worker_count">1</td>
                                    <td><span class="badge badge-success" x-text="site.is_active ? 'Online' : 'Offline'">Online</span></td>
                                    <td class="flex gap-2">
                                        <button @click="editWebsite(site)" class="btn-text text-sm"><i class="fa fa-edit"></i></button>
                                        <button @click="deleteWebsite(site.id)" class="btn-text btn-danger-text text-sm"><i class="fa fa-trash"></i></button>
                                    </td>
                                </tr>
                                <tr x-show="websites.length === 0">
                                    <td colspan="7" class="text-center py-12 text-gray-400">No websites yet. Add your first website.</td>
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
                        <h2 class="text-2xl font-bold text-gray-900">SSL / TLS Certificates</h2>
                        <p class="text-gray-500">Manage Let's Encrypt certificates via ACME</p>
                    </div>
                    <button @click="showAddSslModal = true" class="btn-primary">
                        <i class="fa fa-plus mr-2"></i> Issue Certificate
                    </button>
                </div>

                <div class="bg-white border border-gray-200 rounded-lg overflow-hidden shadow-sm">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th class="w-1/4">Domain</th>
                                    <th class="w-1/5">Issuer</th>
                                    <th class="w-1/5">Expires</th>
                                    <th class="w-1/6">Auto Renew</th>
                                    <th class="w-1/6">Status</th>
                                    <th class="w-1/5">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="cert in sslCerts" :key="cert.id">
                                    <td class="font-mono text-sm text-gray-900" x-text="cert.domain">example.com</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="cert.issuer">letsencrypt</td>
                                    <td class="font-mono text-xs" x-text="cert.expires_at">--</td>
                                    <td><span class="badge" :class="cert.auto_renew ? 'badge-success' : 'badge-default'" x-text="cert.auto_renew ? 'On' : 'Off'">On</span></td>
                                    <td><span class="badge badge-success">Valid</span></td>
                                    <td class="flex gap-2">
                                        <button @click="renewCert(cert.id)" class="btn-text text-sm"><i class="fa fa-sync-alt"></i></button>
                                        <button @click="deleteCert(cert.id)" class="btn-text btn-danger-text text-sm"><i class="fa fa-trash"></i></button>
                                    </td>
                                </tr>
                                <tr x-show="sslCerts.length === 0">
                                    <td colspan="6" class="text-center py-12 text-gray-400">No SSL certificates yet.</td>
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
                        <h2 class="text-2xl font-bold text-gray-900">PHP Workers</h2>
                        <p class="text-gray-500">Manage embedded PHP worker processes</p>
                    </div>
                    <button @click="fetchWorkers()" class="btn-primary">
                        <i class="fa fa-sync mr-2"></i> Refresh
                    </button>
                </div>

                <div class="bg-white border border-gray-200 rounded-lg overflow-hidden shadow-sm">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th>Name</th>
                                    <th class="w-1/6">PID</th>
                                    <th class="w-1/6">Status</th>
                                    <th class="w-1/6">Requests</th>
                                    <th class="w-1/5">Uptime</th>
                                    <th class="w-1/5">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr x-for="worker in workers" :key="worker.id">
                                    <td class="font-mono text-sm text-gray-900" x-text="worker.name">worker-1</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="worker.pid ?? '--'">--</td>
                                    <td><span class="badge" :class="worker.status === 'running' ? 'badge-success' : 'badge-default'" x-text="worker.status">running</span></td>
                                    <td class="font-mono text-sm" x-text="worker.requests">0</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="worker.uptime ?? '--'">--</td>
                                    <td class="flex gap-2">
                                        <button @click="restartWorker(worker.id)" class="btn-text text-sm"><i class="fa fa-redo"></i></button>
                                        <button @click="stopWorker(worker.id)" class="btn-text btn-danger-text text-sm"><i class="fa fa-stop"></i></button>
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
                        <h2 class="text-2xl font-bold text-gray-900">File Manager</h2>
                        <p class="text-gray-500">Browse and edit files on the server</p>
                    </div>
                </div>

                <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                    <div class="flex gap-2 mb-4">
                        <input type="text" class="input-default flex-1" placeholder="Current directory" x-model="fileDir">
                        <button @click="listFiles()" class="btn-default px-4">
                            <i class="fa fa-search"></i>
                        </button>
                    </div>
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr><th class="w-1/3">Name</th><th class="w-1/6">Type</th><th class="w-1/6">Size</th><th class="w-1/4">Modified</th><th>Actions</th></tr>
                            </thead>
                            <tbody>
                                <tr x-for="file in files" :key="file.name">
                                    <td class="font-mono text-sm text-gray-900" x-text="file.name">index.php</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="file.type">file</td>
                                    <td class="font-mono text-xs" x-text="file.size">1.2 KB</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="file.modified">--</td>
                                    <td class="flex gap-2">
                                        <button @click="editFile(file.path)" class="btn-text text-sm"><i class="fa fa-edit"></i></button>
                                    </td>
                                </tr>
                                <tr x-show="files.length === 0">
                                    <td colspan="5" class="text-center py-12 text-gray-400">No files found.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Installer -->
            <div x-show="currentPage === 'installer'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-gray-900">Installer</h2>
                        <p class="text-gray-500">Install third-party services/tools via OS package manager (with binary fallback)</p>
                    </div>
                    <button @click="loadInstaller()" class="btn-default px-4"><i class="fa fa-sync mr-2"></i> Refresh</button>
                </div>
                <div class="bg-white border border-gray-200 rounded-lg overflow-hidden shadow-sm">
                    <table>
                        <thead><tr><th>Service</th><th>Description</th><th>Status</th><th>Actions</th></tr></thead>
                        <tbody>
                            <template x-for="svc in installerServices" :key="svc.id">
                                <tr>
                                    <td class="font-medium text-gray-900" x-text="svc.name"></td>
                                    <td class="text-sm text-gray-500" x-text="svc.description"></td>
                                    <td>
                                        <span class="px-2 py-1 text-xs rounded-full" :class="svc.installed ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-600'" x-text="svc.installed ? 'Installed' : 'Not installed'"></span>
                                    </td>
                                    <td class="flex gap-2">
                                        <button x-show="!svc.installed" @click="installService(svc.id)" class="btn-primary text-xs px-3 py-1">Install</button>
                                        <button x-show="svc.installed" @click="uninstallService(svc.id)" class="btn-text text-xs text-red-600 px-3 py-1">Uninstall</button>
                                    </td>
                                </tr>
                            </template>
                            <tr x-show="installerServices.length === 0"><td colspan="4" class="text-center py-12 text-gray-400">Loading...</td></tr>
                        </tbody>
                    </table>
                </div>
                <div x-show="installerBusy" class="text-sm text-blue-600"><i class="fa fa-spinner fa-spin mr-2"></i>Working, please wait...</div>
                <div x-show="installerMsg" class="text-sm" :class="installerOk ? 'text-green-600' : 'text-red-600'" x-text="installerMsg"></div>
            </div>

            <!-- Users -->
            <div x-show="currentPage === 'users'" class="space-y-6">
                <div class="flex justify-between items-center">
                    <div>
                        <h2 class="text-2xl font-bold text-gray-900">Users</h2>
                        <p class="text-gray-500">Manage panel administrators</p>
                    </div>
                    <button @click="showAddUserModal = true" class="btn-primary">
                        <i class="fa fa-plus mr-2"></i> Add User
                    </button>
                </div>
                <div class="bg-white border border-gray-200 rounded-lg overflow-hidden shadow-sm">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr><th class="w-1/4">Username</th><th class="w-1/5">Role</th><th class="w-1/4">Email</th><th class="w-1/5">Status</th><th>Actions</th></tr>
                            </thead>
                            <tbody>
                                <tr x-for="user in users" :key="user.id">
                                    <td class="font-mono text-sm text-gray-900" x-text="user.username">admin</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="user.role">admin</td>
                                    <td class="font-mono text-xs text-gray-500" x-text="user.email ?? '—'">—</td>
                                    <td><span class="badge badge-success" x-text="user.is_active ? 'Active' : 'Inactive'">Active</span></td>
                                    <td class="flex gap-2"></td>
                                </tr>
                                <tr x-show="users.length === 0">
                                    <td colspan="5" class="text-center py-12 text-gray-400">No users found.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Settings -->
            <div x-show="currentPage === 'settings'" class="space-y-6">
                <h2 class="text-2xl font-bold text-gray-900">Settings</h2>
                <div class="bg-white border border-gray-200 rounded-lg p-6 shadow-sm">
                    <h3 class="ad-card-title">Server Configuration</h3>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div class="form-group">
                            <label class="form-label">Listen Address</label>
                            <input type="text" class="input-default" value="0.0.0.0:8000">
                        </div>
                        <div class="form-group">
                            <label class="form-label">PHP Version</label>
                            <select class="select-default">
                                <option>PHP 8.3</option>
                                <option>PHP 8.2</option>
                                <option>PHP 8.1</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label class="form-label">Worker Count</label>
                            <input type="number" class="input-default" value="4">
                        </div>
                        <div class="form-group">
                            <label class="form-label">Max Body Size</label>
                            <input type="text" class="input-default" value="10 MB">
                        </div>
                    </div>
                    <button class="btn-primary">Save Settings</button>
                </div>
            </div>

        </div>
    </main>

    <!-- Add Website Modal -->
    <div x-show="showAddWebsiteModal" class="modal-overlay" x-transition @click="showAddWebsiteModal = false">
        <button class="modal-close-btn" @click="showAddWebsiteModal = false">&times;</button>
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-gray-900 mb-6">Add Website</h3>
            <form @submit.prevent="addWebsite">
                <div class="form-group">
                    <label class="form-label">Website Name</label>
                    <input type="text" class="input-default" x-model="newWebsite.name" placeholder="My Website" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domain</label>
                    <input type="text" class="input-default" x-model="newWebsite.domain" placeholder="example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Document Root</label>
                    <input type="text" class="input-default" x-model="newWebsite.document_root" placeholder="/var/www/example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">PHP Version</label>
                    <select class="select-default" x-model="newWebsite.php_version">
                        <option>8.3</option><option>8.2</option><option>8.1</option>
                    </select>
                </div>
                <div class="form-group">
                    <label class="form-label">Worker Count</label>
                    <input type="number" class="input-default" x-model="newWebsite.worker_count" value="1" min="1">
                </div>
                <div class="form-group flex items-center gap-2">
                    <input type="checkbox" class="w-4 h-4 border-gray-300 rounded text-blue-600 focus:ring-blue-500" x-model="newWebsite.ssl_enabled">
                    <label class="text-gray-600 text-sm cursor-pointer">Enable SSL (Let's Encrypt)</label>
                </div>
                <div class="form-group" x-show="newWebsite.ssl_enabled">
                    <label class="form-label">ACME Email</label>
                    <input type="email" class="input-default" x-model="newWebsite.acme_email" placeholder="admin@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddWebsiteModal = false" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6">Add Website</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add SSL Modal -->
    <div x-show="showAddSslModal" class="modal-overlay" x-transition @click="showAddSslModal = false">
        <button class="modal-close-btn" @click="showAddSslModal = false">&times;</button>
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-gray-900 mb-6">Issue SSL Certificate</h3>
            <form @submit.prevent="issueSsl">
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-default" x-model="sslForm.email" placeholder="admin@example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domains (comma separated)</label>
                    <input type="text" class="input-default" x-model="sslForm.domains" placeholder="example.com, www.example.com" required>
                </div>
                <div class="text-xs text-gray-500 mt-2">Uses HTTP-01 challenge. Port 80 must be publicly accessible.</div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddSslModal = false" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6">Issue Certificate</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add User Modal -->
    <div x-show="showAddUserModal" class="modal-overlay" x-transition @click="showAddUserModal = false">
        <button class="modal-close-btn" @click="showAddUserModal = false">&times;</button>
        <div class="modal" @click.stop>
            <h3 class="text-xl font-bold text-gray-900 mb-6">Add User</h3>
            <form @submit.prevent="addUser">
                <div class="form-group">
                    <label class="form-label">Username</label>
                    <input type="text" class="input-default" x-model="newUser.username" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Password</label>
                    <input type="password" class="input-default" x-model="newUser.password" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Role</label>
                    <select class="select-default" x-model="newUser.role">
                        <option>admin</option><option>viewer</option>
                    </select>
                </div>
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-default" x-model="newUser.email" placeholder="user@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-4">
                    <button type="button" @click="showAddUserModal = false" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6">Add User</button>
                </div>
            </form>
        </div>
    </div>

    <script>
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
        setInterval(() => {
            const comp = document.querySelector('[x-data]');
            if (comp && comp.__x && comp.__x.currentPage && titles[comp.__x.currentPage]) {
                const h1 = document.querySelector('main h1');
                if (h1) h1.textContent = titles[comp.__x.currentPage];
            }
        }, 100);

        // Initial data load
        setTimeout(() => { fetchWebsites(); fetchSsl(); fetchWorkers(); }, 500);
    </script>
</body>
</html>"#
    .to_string()
}
