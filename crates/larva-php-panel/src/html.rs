//! HTML templates for the control panel.
//! Single-file SPA with Tailwind CSS + Alpine.js.
//! Features the LarvaPHP (Larva x Purple ElePHPant) brand mark, fixed layout, and smooth motion.

/// Main HTML page served at all panel routes (SPA).
pub fn panel_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LarvaPHP Panel</title>
    <link rel="icon" type="image/svg+xml" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 256 256'%3E%3Cdefs%3E%3Cmask id='a'%3E%3Crect width='256' height='256' fill='%23fff'/%3E%3Cpolygon points='82,164 118,124 130,124 94,164' fill='%23000'/%3E%3Cpolygon points='103,168 139,128 151,128 115,168' fill='%23000'/%3E%3C/mask%3E%3Cmask id='b'%3E%3Crect width='256' height='256' fill='%23fff'/%3E%3Ccircle cx='176' cy='122' r='5.5' fill='%23000'/%3E%3C/mask%3E%3C/defs%3E%3Cg transform='translate(-10 4)'%3E%3Cpath d='M92 56C96 36 110 28 128 29' stroke='%23F59E0B' stroke-width='9.5' stroke-linecap='round' fill='none'/%3E%3Ccircle cx='134' cy='29' r='9.5' fill='%23F59E0B'/%3E%3Cpath mask='url(%23a)' fill='%23F59E0B' d='M68 122V90c0-24 14-36 30-36s26 14 26 36v24c0 12 10 18 24 18-6 20-30 32-54 28-18-4-26-20-26-38Z'/%3E%3Crect x='81' y='76' width='11' height='22' rx='5.5' fill='%230F172A'/%3E%3Crect x='100' y='76' width='11' height='22' rx='5.5' fill='%230F172A'/%3E%3Cpath mask='url(%23b)' fill='%236D5DF6' d='M56 124c2 24 16 46 46 48 30 2 48-16 54-38-14-8-20-22-18-36 6-12 24-14 40-4 14 9 20 28 18 48 2 10 10 12 14 0 2-7 8-10 14-8 5 2 6 10 3 19-7 19-27 25-53 17v34c0 8-6 12-14 12h-22c-8 0-12-4-12-12v-10c0-10-26-10-26 0v10c0 8-5 12-13 12H66c-8 0-14-6-14-16v-54c0-10 1-18 4-22Z'/%3E%3C/g%3E%3C/svg%3E">
    <script src="https://cdn.tailwindcss.com"></script>
    <script defer src="https://unpkg.com/alpinejs@3.x/dist/cdn.min.js"></script>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700;800&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>
        :root {
            --ad-primary: #1677ff;
            --ad-primary-hover: #4096ff;
            --ad-primary-active: #0958d9;
            --larva-gold: #f59e0b;
            --php-violet: #6d5df6;
            --ad-bg: #ffffff;
            --ad-bg-secondary: #f4f7fb;
            --ad-text: #0f172a;
            --ad-text-secondary: #475569;
            --ad-text-tertiary: #94a3b8;
            --ad-border: #e2e8f0;
            --ad-border-hover: #6d5df6;
            --ad-success: #52c41a;
            --ad-warning: #faad14;
            --ad-error: #f5222d;
            --ad-radius: 10px;
            --ad-shadow: 0 2px 10px rgba(15, 23, 42, 0.05);
            --ad-shadow-hover: 0 12px 28px -8px rgba(109, 93, 246, 0.16), 0 4px 12px rgba(15, 23, 42, 0.05);
            --ad-font: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
            --ad-mono: 'JetBrains Mono', monospace;
            --sidebar-width: 248px;
            --topbar-height: 68px;
        }

        * { box-sizing: border-box; margin: 0; padding: 0; }
        html { scroll-behavior: smooth; }
        [x-cloak] { display: none !important; }

        body {
            font-family: var(--ad-font);
            background-color: var(--ad-bg-secondary);
            background-image:
                radial-gradient(at 14% 10%, rgba(245, 158, 11, 0.07) 0px, transparent 45%),
                radial-gradient(at 88% 14%, rgba(109, 93, 246, 0.07) 0px, transparent 48%),
                radial-gradient(at 70% 85%, rgba(22, 119, 255, 0.05) 0px, transparent 50%);
            color: var(--ad-text);
            min-height: 100vh;
            line-height: 1.5715;
            overflow-x: hidden;
        }

        .font-mono { font-family: var(--ad-mono); }

        /* Fixed Sidebar & Responsive Layout */
        .ad-sidebar {
            position: fixed;
            top: 0;
            left: 0;
            bottom: 0;
            width: var(--sidebar-width);
            z-index: 40;
            display: flex;
            flex-direction: column;
            background: rgba(255, 255, 255, 0.96);
            backdrop-filter: blur(12px);
            border-right: 1px solid var(--ad-border);
            transition: transform 0.32s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.32s ease;
        }

        .ad-topbar {
            position: fixed;
            top: 0;
            left: var(--sidebar-width);
            right: 0;
            height: var(--topbar-height);
            z-index: 30;
            background: rgba(255, 255, 255, 0.86);
            backdrop-filter: blur(14px);
            -webkit-backdrop-filter: blur(14px);
            border-bottom: 1px solid var(--ad-border);
            transition: left 0.32s cubic-bezier(0.22, 1, 0.36, 1);
        }

        .ad-main {
            margin-left: var(--sidebar-width);
            padding-top: calc(var(--topbar-height) + 28px);
            padding-bottom: 48px;
            min-height: 100vh;
            transition: margin-left 0.32s cubic-bezier(0.22, 1, 0.36, 1);
        }

        .mobile-menu-btn { display: none; }

        .ad-mobile-overlay {
            position: fixed;
            inset: 0;
            background: rgba(15, 23, 42, 0.32);
            backdrop-filter: blur(3px);
            z-index: 39;
        }

        @media (max-width: 768px) {
            .ad-sidebar {
                transform: translateX(-100%);
                box-shadow: none;
            }
            .ad-sidebar.mobile-open {
                transform: translateX(0);
                box-shadow: 0 20px 45px rgba(15, 23, 42, 0.18);
            }
            .ad-topbar { left: 0; }
            .ad-main { margin-left: 0; padding-top: calc(var(--topbar-height) + 20px); }
            .mobile-menu-btn { display: inline-flex; }
        }

        /* Brand Mascot Logo & Motion */
        .ad-logo {
            height: 40px;
            width: 68px;
            border-radius: 11px;
            background: linear-gradient(145deg, #ffffff 0%, #f5f3ff 100%);
            border: 1px solid rgba(109, 93, 246, 0.22);
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 4px 6px;
            box-shadow: 0 6px 16px -4px rgba(109, 93, 246, 0.22), 0 2px 6px rgba(245, 158, 11, 0.14);
            transition: transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1), box-shadow 0.3s ease, border-color 0.3s ease;
        }
        .brand-wrap:hover .ad-logo {
            transform: translateY(-2px) rotate(-4deg) scale(1.07);
            border-color: rgba(109, 93, 246, 0.45);
            box-shadow: 0 10px 22px -5px rgba(109, 93, 246, 0.35), 0 4px 10px rgba(245, 158, 11, 0.2);
        }
        .brand-wrap:hover .mascot-svg {
            animation: mascotBob 0.75s ease-in-out infinite alternate;
        }

        /* Sidebar Navigation Items */
        .nav-btn {
            position: relative;
            width: 100%;
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 9px 14px;
            font-size: 13.5px;
            font-weight: 500;
            color: #475569;
            border-radius: 8px;
            border: none;
            background: transparent;
            cursor: pointer;
            text-align: left;
            transition: all 0.22s cubic-bezier(0.22, 1, 0.36, 1);
            overflow: hidden;
        }
        .nav-btn i {
            width: 20px;
            text-align: center;
            color: #94a3b8;
            transition: transform 0.22s cubic-bezier(0.34, 1.56, 0.64, 1), color 0.22s ease;
        }
        .nav-btn:hover {
            background: #f1f5f9;
            color: #0f172a;
            transform: translateX(3px);
        }
        .nav-btn:hover i {
            color: var(--php-violet);
            transform: scale(1.12);
        }
        .nav-btn.nav-item-active {
            background: linear-gradient(90deg, rgba(109, 93, 246, 0.12) 0%, rgba(22, 119, 255, 0.04) 100%);
            color: #5b4ce6;
            font-weight: 600;
            transform: translateX(0);
        }
        .nav-btn.nav-item-active::before {
            content: '';
            position: absolute;
            left: 0;
            top: 6px;
            bottom: 6px;
            width: 3.5px;
            border-radius: 0 4px 4px 0;
            background: linear-gradient(180deg, var(--larva-gold), var(--php-violet));
            animation: pillIn 0.25s cubic-bezier(0.22, 1, 0.36, 1) forwards;
        }
        .nav-btn.nav-item-active i {
            color: var(--php-violet);
        }

        /* Typography */
        .page-title { font-size: 19px; font-weight: 700; color: #0f172a; letter-spacing: -0.015em; line-height: 1.25; }
        .page-subtitle { font-size: 13px; color: #64748b; margin-top: 1px; }

        /* Cards & Interactive Surfaces */
        .fancy-card {
            background: rgba(255, 255, 255, 0.94);
            border: 1px solid var(--ad-border);
            border-radius: 12px;
            box-shadow: var(--ad-shadow);
            transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.28s ease, border-color 0.28s ease;
            position: relative;
            overflow: hidden;
        }
        .fancy-card-hover:hover {
            transform: translateY(-4px);
            box-shadow: var(--ad-shadow-hover);
            border-color: rgba(109, 93, 246, 0.32);
        }
        .stat-card::after {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 3px;
            background: linear-gradient(90deg, var(--card-accent, #6d5df6), var(--card-accent-end, #f59e0b));
            opacity: 0;
            transform: scaleX(0.35);
            transition: opacity 0.28s ease, transform 0.32s cubic-bezier(0.22, 1, 0.36, 1);
        }
        .stat-card:hover::after {
            opacity: 1;
            transform: scaleX(1);
        }

        .stat-card-value { font-size: 28px; font-weight: 700; letter-spacing: -0.025em; line-height: 1.2; margin-top: 8px; }
        .stat-card-label { font-size: 11px; font-weight: 600; color: #64748b; text-transform: uppercase; letter-spacing: 0.06em; }
        .ad-card-title { font-size: 14px; font-weight: 600; color: #0f172a; margin-bottom: 14px; }

        /* Progress Bars with Animated Shimmer */
        .progress-h {
            height: 9px;
            border-radius: 999px;
            background: #eef2f7;
            overflow: hidden;
            position: relative;
        }
        .progress-fill {
            height: 100%;
            border-radius: 999px;
            transition: width 0.75s cubic-bezier(0.22, 1, 0.36, 1);
            position: relative;
            overflow: hidden;
        }
        .progress-fill::after {
            content: '';
            position: absolute;
            inset: 0;
            background: linear-gradient(90deg, transparent 0%, rgba(255, 255, 255, 0.38) 50%, transparent 100%);
            transform: translateX(-100%);
            animation: shimmerSlide 2.4s infinite ease-in-out;
        }

        /* Badges */
        .badge {
            display: inline-flex;
            align-items: center;
            gap: 5px;
            padding: 3px 10px;
            border-radius: 999px;
            font-size: 11px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.03em;
        }
        .badge-default { background: #f1f5f9; color: #64748b; border: 1px solid #e2e8f0; }
        .badge-primary { background: #eef2ff; color: #5b4ce6; border: 1px solid #c7d2fe; }
        .badge-success { background: #f6ffed; color: #389e0d; border: 1px solid #b7eb8f; }
        .badge-warning { background: #fffbe6; color: #d48806; border: 1px solid #ffe58f; }
        .badge-error { background: #fff2f0; color: #cf1322; border: 1px solid #ffccc7; }

        /* Floating Toast Alert */
        .toast-alert {
            position: fixed;
            top: 84px;
            right: 24px;
            z-index: 60;
            min-width: 280px;
            max-width: 420px;
            padding: 12px 16px;
            border-radius: 10px;
            font-size: 14px;
            font-weight: 500;
            box-shadow: 0 14px 30px -6px rgba(15, 23, 42, 0.16);
            backdrop-filter: blur(8px);
            display: flex;
            align-items: center;
            gap: 10px;
        }
        .alert-info { background: rgba(238, 242, 255, 0.96); border: 1px solid #c7d2fe; color: #4338ca; }
        .alert-success { background: rgba(246, 255, 237, 0.96); border: 1px solid #b7eb8f; color: #389e0d; }
        .alert-error { background: rgba(255, 241, 240, 0.96); border: 1px solid #ffa39e; color: #cf1322; }

        /* Tables */
        .table-wrapper { overflow-x: auto; }
        table { width: 100%; border-collapse: collapse; }
        th {
            text-align: left;
            padding: 13px 18px;
            font-size: 11px;
            font-weight: 600;
            color: #64748b;
            background: #f8fafc;
            text-transform: uppercase;
            letter-spacing: 0.06em;
            border-bottom: 1px solid #e2e8f0;
        }
        td {
            padding: 14px 18px;
            border-bottom: 1px solid #f1f5f9;
            color: #0f172a;
            font-size: 14px;
            transition: background-color 0.18s ease;
        }
        tbody tr { transition: transform 0.18s ease, background-color 0.18s ease; }
        tbody tr:hover td { background: #f8faff; }
        tbody tr:last-child td { border-bottom: none; }

        /* Buttons with Shine & Tactile Press */
        .btn-primary {
            position: relative;
            overflow: hidden;
            background: linear-gradient(135deg, #6d5df6 0%, #4f46e5 100%);
            color: #fff;
            border: none;
            border-radius: 8px;
            padding: 9px 20px;
            font-weight: 500;
            font-size: 14px;
            cursor: pointer;
            box-shadow: 0 4px 12px -2px rgba(109, 93, 246, 0.36);
            transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.2s ease, filter 0.2s ease;
        }
        .btn-primary::before {
            content: '';
            position: absolute;
            top: 0;
            left: -100%;
            width: 60%;
            height: 100%;
            background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.26), transparent);
            transform: skewX(-20deg);
            transition: left 0.55s ease;
        }
        .btn-primary:hover {
            transform: translateY(-1.5px);
            box-shadow: 0 8px 18px -3px rgba(109, 93, 246, 0.45);
            filter: brightness(1.06);
        }
        .btn-primary:hover::before { left: 140%; }
        .btn-primary:active { transform: translateY(0) scale(0.97); }

        .btn-default {
            background: #fff;
            color: #1e293b;
            border: 1px solid #cbd5e1;
            border-radius: 8px;
            padding: 9px 18px;
            font-weight: 500;
            font-size: 14px;
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.22, 1, 0.36, 1);
        }
        .btn-default:hover {
            border-color: var(--php-violet);
            color: var(--php-violet);
            background: #f5f3ff;
            transform: translateY(-1.5px);
            box-shadow: 0 4px 12px rgba(109, 93, 246, 0.12);
        }
        .btn-default:active { transform: translateY(0) scale(0.97); }

        .btn-text {
            background: none;
            border: none;
            color: var(--php-violet);
            font-size: 14px;
            cursor: pointer;
            padding: 6px 10px;
            border-radius: 6px;
            transition: background-color 0.18s ease, transform 0.18s ease;
        }
        .btn-text:hover { background: #eef2ff; transform: scale(1.06); }
        .btn-danger-text { color: #ef4444; }
        .btn-danger-text:hover { background: #fef2f2; color: #dc2626; }

        /* Inputs & Selects */
        .input-default, .select-default {
            border: 1px solid #cbd5e1;
            border-radius: 8px;
            padding: 9px 12px;
            font-size: 14px;
            width: 100%;
            background: #fff;
            color: #0f172a;
            transition: border-color 0.2s ease, box-shadow 0.2s ease;
            outline: none;
        }
        .input-default:focus, .select-default:focus {
            border-color: var(--php-violet);
            box-shadow: 0 0 0 3px rgba(109, 93, 246, 0.15);
        }
        .select-default { cursor: pointer; }
        .form-label { display: block; margin-bottom: 6px; font-size: 13px; font-weight: 600; color: #334155; }
        .form-group { margin-bottom: 18px; }

        /* Modals */
        .modal-overlay {
            position: fixed;
            inset: 0;
            background: rgba(15, 23, 42, 0.42);
            z-index: 50;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 16px;
            backdrop-filter: blur(4px);
        }
        .modal {
            position: relative;
            background: #fff;
            border: 1px solid var(--ad-border);
            border-radius: 14px;
            padding: 26px;
            width: 100%;
            max-width: 480px;
            max-height: 90vh;
            overflow-y: auto;
            box-shadow: 0 24px 48px -12px rgba(15, 23, 42, 0.25);
        }
        .modal-close-btn {
            position: absolute;
            right: 16px;
            top: 16px;
            width: 32px;
            height: 32px;
            border-radius: 8px;
            background: #f8fafc;
            border: 1px solid #e2e8f0;
            color: #64748b;
            cursor: pointer;
            font-size: 18px;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s ease;
        }
        .modal-close-btn:hover {
            color: #0f172a;
            background: #f1f5f9;
            transform: rotate(90deg);
        }

        /* Quick Action Tiles */
        .quick-action-btn {
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 14px 16px;
            border-radius: 10px;
            border: 1px solid var(--ad-border);
            background: #fff;
            cursor: pointer;
            text-align: left;
            transition: all 0.25s cubic-bezier(0.22, 1, 0.36, 1);
        }
        .quick-action-btn:hover {
            transform: translateY(-3px);
            border-color: #a5b4fc;
            box-shadow: 0 10px 22px -6px rgba(109, 93, 246, 0.18);
        }
        .quick-action-btn:hover .qa-icon {
            transform: scale(1.1) rotate(-4deg);
        }
        .qa-icon {
            width: 38px;
            height: 38px;
            border-radius: 9px;
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
            transition: transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
        }

        /* Live Ping & Notification Bell */
        .online-dot {
            position: relative;
            width: 8px;
            height: 8px;
            border-radius: 999px;
            background: #22c55e;
        }
        .online-dot::after {
            content: '';
            position: absolute;
            inset: -3px;
            border-radius: 999px;
            border: 2px solid #22c55e;
            animation: radarPulse 2s cubic-bezier(0, 0, 0.2, 1) infinite;
        }
        .bell-btn:hover i { animation: bellRing 0.55s ease; }

        /* Keyframe Animations */
        @keyframes fadeSlideUp {
            from { opacity: 0; transform: translateY(14px); }
            to { opacity: 1; transform: translateY(0); }
        }
        @keyframes pillIn {
            from { transform: scaleY(0.2); opacity: 0; }
            to { transform: scaleY(1); opacity: 1; }
        }
        @keyframes shimmerSlide {
            0% { transform: translateX(-100%); }
            60%, 100% { transform: translateX(200%); }
        }
        @keyframes radarPulse {
            0% { transform: scale(0.8); opacity: 0.85; }
            80%, 100% { transform: scale(2.1); opacity: 0; }
        }
        @keyframes mascotBob {
            from { transform: translateY(1px); }
            to { transform: translateY(-2px); }
        }
        @keyframes bellRing {
            0%, 100% { transform: rotate(0); }
            20% { transform: rotate(14deg); }
            40% { transform: rotate(-12deg); }
            60% { transform: rotate(8deg); }
            80% { transform: rotate(-6deg); }
        }
        @keyframes adspin {
            from { transform: rotate(0deg); }
            to { transform: rotate(360deg); }
        }

        .animate-spin { animation: adspin 0.9s linear infinite; }
        .stagger-in { animation: fadeSlideUp 0.45s cubic-bezier(0.22, 1, 0.36, 1) backwards; }
        .delay-1 { animation-delay: 0.04s; }
        .delay-2 { animation-delay: 0.08s; }
        .delay-3 { animation-delay: 0.12s; }
        .delay-4 { animation-delay: 0.16s; }
        .delay-5 { animation-delay: 0.20s; }
        .delay-6 { animation-delay: 0.24s; }

        @media (prefers-reduced-motion: reduce) {
            *, *::before, *::after {
                animation-duration: 0.01ms !important;
                animation-iteration-count: 1 !important;
                transition-duration: 0.01ms !important;
            }
        }
    </style>
</head>
<body x-data="larvaPanel()" x-init="init()" @keydown.escape.window="closeModal()" class="min-h-screen">

    <!-- Mobile overlay -->
    <div x-cloak x-show="sidebarOpen"
         x-transition:enter="transition ease-out duration-200"
         x-transition:enter-start="opacity-0"
         x-transition:enter-end="opacity-100"
         x-transition:leave="transition ease-in duration-150"
         x-transition:leave-start="opacity-100"
         x-transition:leave-end="opacity-0"
         @click="sidebarOpen = false"
         class="ad-mobile-overlay"></div>

    <!-- Fixed Sidebar -->
    <aside class="ad-sidebar" :class="{ 'mobile-open': sidebarOpen }">
        <div class="px-4 h-[68px] flex items-center gap-2.5 border-b border-slate-100 brand-wrap cursor-pointer shrink-0" @click="navigate('dashboard')">
            <div class="ad-logo">
                <svg class="mascot-svg w-full h-full" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 200" fill="none" aria-label="LarvaPHP Mascot Logo">
                    <defs>
                        <mask id="sb-php-larva">
                            <rect width="320" height="200" fill="#ffffff"/>
                            <path d="M58 76 H70 C77 76, 82 81, 82 88 C82 95, 77 100, 70 100 H58 Z" fill="#000000"/>
                            <polygon points="16,172 60,126 70,126 26,172" fill="#000000"/>
                            <polygon points="28,186 72,140 82,140 38,186" fill="#000000"/>
                        </mask>
                        <mask id="sb-php-elephant">
                            <rect width="320" height="200" fill="#ffffff"/>
                            <circle cx="256" cy="84" r="5" fill="#000000"/>
                            <path d="M238 122 C248 118, 258 124, 260 134 C252 138, 242 136, 238 130 Z" fill="#000000"/>
                        </mask>
                    </defs>
                    <path d="M48 48 C52 28, 64 20, 78 22" stroke="#F59E0B" stroke-width="8.5" stroke-linecap="round"/>
                    <circle cx="84" cy="22" r="9" fill="#F59E0B"/>
                    <path mask="url(#sb-php-larva)" fill="#F59E0B" d="M20 156 V74 C20 55, 34 46, 56 46 H72 C93 46, 106 62, 106 86 C106 110, 93 126, 72 126 H58 V156 C58 174, 46 184, 34 184 C24 184, 20 172, 20 156 Z"/>
                    <rect x="30" y="72" width="9" height="19" rx="4.5" fill="#0F172A"/>
                    <rect x="43" y="72" width="9" height="19" rx="4.5" fill="#0F172A"/>
                    <path fill="#F59E0B" d="M116 56 C116 50, 120 46, 126 46 H134 C140 46, 144 50, 144 56 V102 H158 L144 126 V174 C144 180, 140 184, 134 184 H126 C120 184, 116 180, 116 174 Z"/>
                    <path fill="#6D5DF6" d="M170 56 C170 50, 174 46, 180 46 H188 C194 46, 198 50, 198 56 V174 C198 180, 194 184, 188 184 H180 C174 184, 170 180, 170 174 V126 H152 L166 102 H170 Z"/>
                    <path mask="url(#sb-php-elephant)" fill="#6D5DF6" d="M210 74 C212 56, 226 46, 248 46 C274 46, 288 64, 286 92 C285 110, 290 122, 297 120 C302 118, 304 111, 304 106 C304 101, 312 100, 314 106 C317 118, 312 138, 294 142 C276 146, 260 134, 246 130 H238 V174 C238 180, 234 184, 228 184 H218 C212 184, 208 180, 208 174 V92 C208 84, 209 78, 210 74 Z"/>
                </svg>
            </div>
            <div>
                <span class="font-extrabold text-[17px] text-slate-900 tracking-tight block leading-none">Larva<span style="color:#6d5df6">PHP</span></span>
                <span class="text-[11px] font-medium text-slate-400 tracking-wide">Control Panel</span>
            </div>
        </div>

        <nav class="flex-1 overflow-y-auto px-3 py-4 space-y-1">
            <div class="text-[11px] font-bold text-slate-400 uppercase tracking-wider px-3 mb-2">Navigation</div>

            <button @click="navigate('dashboard')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'dashboard' }">
                <i class="fa fa-chart-line"></i> <span>Dashboard</span>
            </button>
            <button @click="navigate('websites')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'websites' }">
                <i class="fa fa-globe"></i> <span>Websites</span>
            </button>
            <button @click="navigate('ssl')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'ssl' }">
                <i class="fa fa-lock"></i> <span>SSL / TLS</span>
            </button>
            <button @click="navigate('workers')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'workers' }">
                <i class="fa fa-cogs"></i> <span>PHP Workers</span>
            </button>
            <button @click="navigate('files')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'files' }">
                <i class="fa fa-folder"></i> <span>File Manager</span>
            </button>
            <button @click="navigate('installer')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'installer' }">
                <i class="fa fa-download"></i> <span>Installer</span>
            </button>
            <button @click="navigate('mysql')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'mysql' }">
                <i class="fa fa-database"></i> <span>MySQL Accounts</span>
            </button>
            <button @click="navigate('postgres')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'postgres' }">
                <i class="fa fa-database"></i> <span>PostgreSQL Accounts</span>
            </button>
            <button @click="navigate('ftp')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'ftp' }">
                <i class="fa fa-folder-open"></i> <span>FTP Accounts</span>
            </button>
            <button @click="navigate('users')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'users' }">
                <i class="fa fa-users"></i> <span>Users</span>
            </button>
            <button @click="navigate('settings')" class="nav-btn" :class="{ 'nav-item-active': currentPage === 'settings' }">
                <i class="fa fa-sliders-h"></i> <span>Settings</span>
            </button>
        </nav>

        <div class="mt-auto p-3.5 border-t border-slate-100 shrink-0">
            <button @click="sidebarOpen = false; showAlert('Session active', 'info')" class="w-full btn-text text-sm text-slate-600 hover:text-red-600 hover:bg-red-50 flex items-center gap-2.5 px-3 py-2 rounded-lg">
                <i class="fa fa-power-off"></i> <span>Logout</span>
            </button>
        </div>
    </aside>

    <!-- Fixed Topbar -->
    <header class="ad-topbar flex items-center justify-between px-4 sm:px-6">
        <div class="flex items-center gap-3">
            <button @click="sidebarOpen = !sidebarOpen" class="mobile-menu-btn items-center justify-center w-9 h-9 rounded-lg text-slate-600 hover:bg-slate-100 transition-colors" aria-label="Toggle Menu">
                <i class="fa fa-bars text-lg"></i>
            </button>
            <div>
                <h1 class="page-title" x-text="pageMeta.title">Dashboard</h1>
                <p class="page-subtitle" x-text="pageMeta.subtitle">Server overview and performance metrics</p>
            </div>
        </div>
        <div class="flex items-center gap-3">
            <button @click="fetchStats()" title="Refresh metrics" class="bell-btn w-9 h-9 rounded-lg flex items-center justify-center text-slate-500 hover:text-indigo-600 hover:bg-indigo-50 transition-colors relative">
                <i class="fa fa-bell text-base"></i>
                <span x-show="alert" class="absolute top-2 right-2 w-2 h-2 bg-indigo-500 rounded-full"></span>
            </button>
            <div class="flex items-center gap-2 px-3 py-1.5 bg-emerald-50/80 rounded-full text-xs border border-emerald-200/80">
                <span class="online-dot"></span>
                <span class="font-semibold text-emerald-700">Online</span>
            </div>
        </div>
    </header>

    <!-- Floating Toast Alert -->
    <div x-cloak x-show="alert"
         x-transition:enter="transition ease-out duration-300"
         x-transition:enter-start="opacity-0 translate-y-[-12px] scale-95"
         x-transition:enter-end="opacity-100 translate-y-0 scale-100"
         x-transition:leave="transition ease-in duration-200"
         x-transition:leave-start="opacity-100 translate-y-0 scale-100"
         x-transition:leave-end="opacity-0 translate-y-[-10px] scale-95"
         class="toast-alert"
         :class="`alert-${alertType}`">
        <i class="fa" :class="alertType === 'success' ? 'fa-check-circle' : (alertType === 'error' ? 'fa-exclamation-circle' : 'fa-info-circle')"></i>
        <span class="flex-1" x-text="alert"></span>
        <button @click="alert = null" class="opacity-60 hover:opacity-100 text-base leading-none">&times;</button>
    </div>

    <!-- Main Content -->
    <main class="ad-main">
        <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">

            <!-- Dashboard -->
            <div x-show="currentPage === 'dashboard'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-6">

                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="flex items-center gap-2 text-xs text-slate-500">
                        <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-white border border-slate-200 font-medium text-slate-600">
                            <i class="fa fa-server text-indigo-500"></i> Live Telemetry
                        </span>
                        <span x-show="lastUpdated" class="font-mono" x-text="'Updated ' + lastUpdated"></span>
                    </div>
                    <button @click="refreshAll()" class="btn-default text-sm inline-flex items-center gap-2">
                        <i class="fa fa-sync-alt" :class="{ 'animate-spin': isRefreshing }"></i>
                        <span x-text="isRefreshing ? 'Refreshing...' : 'Refresh'"></span>
                    </button>
                    <button @click="forceReloadServer()" class="btn-primary text-sm inline-flex items-center gap-2">
                        <i class="fa fa-redo"></i> Force Reload Server
                    </button>
                </div>

                <!-- Stats Grid -->
                <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4">
                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-1" style="--card-accent: #1677ff; --card-accent-end: #60a5fa;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">Total Websites</div>
                            <span class="w-8 h-8 rounded-lg bg-blue-50 text-blue-600 flex items-center justify-center text-xs"><i class="fa fa-globe"></i></span>
                        </div>
                        <div class="stat-card-value text-blue-600" x-text="stats.total_websites">0</div>
                    </div>

                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-2" style="--card-accent: #16a34a; --card-accent-end: #4ade80;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">Active Websites</div>
                            <span class="w-8 h-8 rounded-lg bg-emerald-50 text-emerald-600 flex items-center justify-center text-xs"><i class="fa fa-check"></i></span>
                        </div>
                        <div class="stat-card-value text-emerald-600" x-text="stats.active_websites">0</div>
                    </div>

                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-3" style="--card-accent: #6d5df6; --card-accent-end: #a78bfa;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">PHP Workers</div>
                            <span class="w-8 h-8 rounded-lg bg-indigo-50 text-indigo-600 flex items-center justify-center text-xs"><i class="fa fa-cogs"></i></span>
                        </div>
                        <div class="stat-card-value text-indigo-600" x-text="stats.total_workers">0</div>
                    </div>

                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-4" style="--card-accent: #16a34a; --card-accent-end: #34d399;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">Running</div>
                            <span class="w-8 h-8 rounded-lg bg-green-50 text-green-600 flex items-center justify-center text-xs"><i class="fa fa-bolt"></i></span>
                        </div>
                        <div class="stat-card-value text-green-600" x-text="stats.running_workers">0</div>
                    </div>

                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-5" style="--card-accent: #f59e0b; --card-accent-end: #fbbf24;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">SSL Certs</div>
                            <span class="w-8 h-8 rounded-lg bg-amber-50 text-amber-600 flex items-center justify-center text-xs"><i class="fa fa-lock"></i></span>
                        </div>
                        <div class="stat-card-value text-amber-600" x-text="stats.ssl_certificates">0</div>
                    </div>

                    <div class="fancy-card fancy-card-hover stat-card p-5 stagger-in delay-6" style="--card-accent: #334155; --card-accent-end: #64748b;">
                        <div class="flex items-center justify-between">
                            <div class="stat-card-label">Requests Today</div>
                            <span class="w-8 h-8 rounded-lg bg-slate-100 text-slate-700 flex items-center justify-center text-xs"><i class="fa fa-wave-square"></i></span>
                        </div>
                        <div class="stat-card-value text-slate-900" x-text="stats.requests_today">0</div>
                    </div>
                </div>

                <!-- Resource Usage -->
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-5">
                    <div class="fancy-card fancy-card-hover p-6 stagger-in delay-2">
                        <div class="ad-card-title flex items-center justify-between">
                            <span class="flex items-center gap-2"><i class="fa fa-microchip text-amber-500"></i> CPU Usage</span>
                            <span class="text-xs font-mono px-2 py-0.5 rounded bg-amber-50 text-amber-700" x-text="Number(stats.cpu_usage || 0).toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="flex items-baseline justify-between mb-3">
                            <span class="text-2xl font-bold text-slate-900" x-text="Math.round(stats.cpu_usage || 0) + '%'">0%</span>
                            <span class="text-xs text-slate-400">Processor load</span>
                        </div>
                        <div class="progress-h">
                            <div class="progress-fill bg-gradient-to-r from-amber-400 to-amber-500" :style="`width: ${Math.min(100, Math.max(0, stats.cpu_usage || 0))}%`"></div>
                        </div>
                    </div>

                    <div class="fancy-card fancy-card-hover p-6 stagger-in delay-3">
                        <div class="ad-card-title flex items-center justify-between">
                            <span class="flex items-center gap-2"><i class="fa fa-memory text-indigo-600"></i> Memory Usage</span>
                            <span class="text-xs font-mono px-2 py-0.5 rounded bg-indigo-50 text-indigo-700" x-text="Number(stats.memory_usage || 0).toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="flex items-baseline justify-between mb-3">
                            <span class="text-2xl font-bold text-slate-900" x-text="Math.round(stats.memory_usage || 0) + '%'">0%</span>
                            <span class="text-xs text-slate-400">RAM allocation</span>
                        </div>
                        <div class="progress-h">
                            <div class="progress-fill bg-gradient-to-r from-indigo-500 to-violet-600" :style="`width: ${Math.min(100, Math.max(0, stats.memory_usage || 0))}%`"></div>
                        </div>
                    </div>

                    <div class="fancy-card fancy-card-hover p-6 stagger-in delay-4">
                        <div class="ad-card-title flex items-center justify-between">
                            <span class="flex items-center gap-2"><i class="fa fa-hdd text-emerald-600"></i> Disk Usage</span>
                            <span class="text-xs font-mono px-2 py-0.5 rounded bg-emerald-50 text-emerald-700" x-text="Number(stats.disk_usage || 0).toFixed(1) + '%'">0.0%</span>
                        </div>
                        <div class="flex items-baseline justify-between mb-3">
                            <span class="text-2xl font-bold text-slate-900" x-text="Math.round(stats.disk_usage || 0) + '%'">0%</span>
                            <span class="text-xs text-slate-400">Storage volume</span>
                        </div>
                        <div class="progress-h">
                            <div class="progress-fill bg-gradient-to-r from-emerald-500 to-teal-500" :style="`width: ${Math.min(100, Math.max(0, stats.disk_usage || 0))}%`"></div>
                        </div>
                    </div>
                </div>

                <!-- Quick Actions -->
                <div class="fancy-card p-6 stagger-in delay-5">
                    <div class="flex items-center justify-between mb-5">
                        <h3 class="text-slate-900 font-semibold text-base">Quick Actions</h3>
                        <span class="text-xs text-slate-400">Common server workflows</span>
                    </div>
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
                        <button @click="openNewWebsiteModal()" class="quick-action-btn">
                            <span class="qa-icon bg-amber-500 text-white shadow-sm"><i class="fa fa-plus"></i></span>
                            <div>
                                <div class="text-sm font-semibold text-slate-900">Add Website</div>
                                <div class="text-xs text-slate-500">Create a new PHP vhost</div>
                            </div>
                        </button>
                        <button @click="navigate('ssl'); showAddSslModal = true" class="quick-action-btn">
                            <span class="qa-icon bg-indigo-600 text-white shadow-sm"><i class="fa fa-lock"></i></span>
                            <div>
                                <div class="text-sm font-semibold text-slate-900">Issue SSL</div>
                                <div class="text-xs text-slate-500">Let's Encrypt ACME cert</div>
                            </div>
                        </button>
                        <button @click="fetchWorkers(); showAlert('Workers synced', 'success')" class="quick-action-btn">
                            <span class="qa-icon bg-indigo-50 text-indigo-600"><i class="fa fa-sync"></i></span>
                            <div>
                                <div class="text-sm font-semibold text-slate-900">Sync Workers</div>
                                <div class="text-xs text-slate-500">Refresh PHP runtime pool</div>
                            </div>
                        </button>
                        <button @click="showAddUserModal = true" class="quick-action-btn">
                            <span class="qa-icon bg-emerald-50 text-emerald-600"><i class="fa fa-user-plus"></i></span>
                            <div>
                                <div class="text-sm font-semibold text-slate-900">Add User</div>
                                <div class="text-xs text-slate-500">Grant panel access</div>
                            </div>
                        </button>
                    </div>
                </div>
            </div>

            <!-- Websites -->
            <div x-cloak x-show="currentPage === 'websites'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500" x-text="`${websites.length} website(s) configured`"></div>
                    <button @click="openNewWebsiteModal()" class="btn-primary inline-flex items-center gap-2">
                        <i class="fa fa-plus"></i> Add Website
                    </button>
                </div>

                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr>
                                    <th>Domain</th>
                                    <th>Document Root</th>
                                    <th>PHP Version</th>
                                    <th>SSL</th>
                                    <th>Workers</th>
                                    <th>Port</th>
                                    <th>Status</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                <template x-for="site in websites" :key="site.id">
                                    <tr>
                                        <td>
                                            <div class="font-semibold text-slate-900" x-text="site.name"></div>
                                            <div class="font-mono text-xs text-indigo-600" x-text="site.domain"></div>
                                        </td>
                                        <td class="font-mono text-xs text-slate-500" x-text="site.document_root"></td>
                                        <td><span class="badge badge-primary font-mono" x-text="'PHP ' + site.php_version"></span></td>
                                        <td><span class="badge" :class="site.ssl_enabled ? 'badge-success' : 'badge-default'" x-text="site.ssl_enabled ? 'Active' : 'Inactive'"></span></td>
                                        <td class="font-mono text-sm" x-text="site.worker_count"></td>
                                        <td class="font-mono text-sm" x-text="site.listen_port || 'default'"></td>
                                        <td><span class="badge" :class="site.is_active ? 'badge-success' : 'badge-default'" x-text="site.is_active ? 'Online' : 'Offline'"></span></td>
                                        <td>
                                            <div class="flex gap-1">
                                                <button @click="editWebsite(site)" class="btn-text text-sm" title="Edit"><i class="fa fa-edit"></i></button>
                                                <button @click="deleteWebsite(site.id)" class="btn-text btn-danger-text text-sm" title="Delete"><i class="fa fa-trash"></i></button>
                                            </div>
                                        </td>
                                    </tr>
                                </template>
                                <tr x-show="websites.length === 0">
                                    <td colspan="7" class="text-center py-14 text-slate-400">
                                        <i class="fa fa-globe text-3xl mb-2 block opacity-40"></i>
                                        No websites yet. Click <strong>Add Website</strong> to create one.
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- SSL / TLS -->
            <div x-cloak x-show="currentPage === 'ssl'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500" x-text="`${sslCerts.length} certificate(s) issued`"></div>
                    <button @click="showAddSslModal = true" class="btn-primary inline-flex items-center gap-2">
                        <i class="fa fa-plus"></i> Issue Certificate
                    </button>
                </div>

                <div class="fancy-card">
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
                                <template x-for="cert in sslCerts" :key="cert.id">
                                    <tr>
                                        <td class="font-mono text-sm font-medium text-slate-900" x-text="cert.domain"></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="cert.issuer"></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="cert.expires_at || '--'"></td>
                                        <td><span class="badge" :class="cert.auto_renew ? 'badge-success' : 'badge-default'" x-text="cert.auto_renew ? 'On' : 'Off'"></span></td>
                                        <td><span class="badge badge-success">Valid</span></td>
                                        <td>
                                            <div class="flex gap-1">
                                                <button @click="renewCert(cert.id)" class="btn-text text-sm" title="Renew"><i class="fa fa-sync-alt"></i></button>
                                                <button @click="deleteCert(cert.id)" class="btn-text btn-danger-text text-sm" title="Delete"><i class="fa fa-trash"></i></button>
                                            </div>
                                        </td>
                                    </tr>
                                </template>
                                <tr x-show="sslCerts.length === 0">
                                    <td colspan="6" class="text-center py-14 text-slate-400">
                                        <i class="fa fa-shield-alt text-3xl mb-2 block opacity-40"></i>
                                        No SSL certificates yet.
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- PHP Workers -->
            <div x-cloak x-show="currentPage === 'workers'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500" x-text="`${workers.length} active worker(s)`"></div>
                    <button @click="fetchWorkers()" class="btn-primary inline-flex items-center gap-2">
                        <i class="fa fa-sync"></i> Refresh
                    </button>
                </div>

                <div class="fancy-card">
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
                                <template x-for="worker in workers" :key="worker.id">
                                    <tr>
                                        <td class="font-mono text-sm font-medium text-slate-900" x-text="worker.name"></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="worker.pid ?? '--'"></td>
                                        <td><span class="badge" :class="worker.status === 'running' ? 'badge-success' : 'badge-default'" x-text="worker.status"></span></td>
                                        <td class="font-mono text-sm" x-text="worker.requests"></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="worker.uptime ?? '--'"></td>
                                        <td>
                                            <div class="flex gap-1">
                                                <button @click="restartWorker(worker.id)" class="btn-text text-sm" title="Restart"><i class="fa fa-redo"></i></button>
                                                <button @click="stopWorker(worker.id)" class="btn-text btn-danger-text text-sm" title="Stop"><i class="fa fa-stop"></i></button>
                                            </div>
                                        </td>
                                    </tr>
                                </template>
                                <tr x-show="workers.length === 0">
                                    <td colspan="6" class="text-center py-14 text-slate-400">
                                        <i class="fa fa-cogs text-3xl mb-2 block opacity-40"></i>
                                        No running PHP workers reported yet.
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- File Manager -->
            <div x-cloak x-show="currentPage === 'files'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="fancy-card p-5">
                    <div class="flex gap-2.5 mb-5">
                        <input type="text" class="input-default flex-1 font-mono text-sm" placeholder="Current directory" x-model="fileDir" @keydown.enter="listFiles()">
                        <button @click="listFiles()" class="btn-primary px-5 inline-flex items-center gap-2">
                            <i class="fa fa-search"></i> Browse
                        </button>
                    </div>
                    <div class="table-wrapper border border-slate-200 rounded-lg overflow-hidden">
                        <table>
                            <thead>
                                <tr><th>Name</th><th>Type</th><th>Size</th><th>Modified</th><th>Actions</th></tr>
                            </thead>
                            <tbody>
                                <template x-for="file in files" :key="file.name">
                                    <tr>
                                        <td class="font-mono text-sm text-slate-900">
                                            <i class="fa mr-2 text-indigo-500" :class="file.type === 'dir' ? 'fa-folder' : 'fa-file-code'"></i>
                                            <span x-text="file.name"></span>
                                        </td>
                                        <td class="font-mono text-xs text-slate-500" x-text="file.type"></td>
                                        <td class="font-mono text-xs" x-text="file.size"></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="file.modified || '--'"></td>
                                        <td>
                                            <button @click="inspectFile(file.path)" class="btn-text text-sm" title="Inspect"><i class="fa fa-edit"></i></button>
                                        </td>
                                    </tr>
                                </template>
                                <tr x-show="files.length === 0">
                                    <td colspan="5" class="text-center py-14 text-slate-400">
                                        <i class="fa fa-folder-open text-3xl mb-2 block opacity-40"></i>
                                        No files found. Click <strong>Browse</strong> to load directory.
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Installer -->
            <div x-cloak x-show="currentPage === 'installer'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500">Install third-party services/tools via OS package manager (with binary fallback)</div>
                    <button @click="loadInstaller()" class="btn-default inline-flex items-center gap-2">
                        <i class="fa fa-sync" :class="{ 'animate-spin': installerBusy }"></i> Refresh
                    </button>
                </div>
                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead><tr><th>Service</th><th>Description</th><th>Status</th><th>Actions</th></tr></thead>
                            <tbody>
                                <template x-for="svc in installerServices" :key="svc.id">
                                    <tr>
                                        <td class="font-semibold text-slate-900" x-text="svc.name"></td>
                                        <td class="text-sm text-slate-500" x-text="svc.description"></td>
                                        <td>
                                            <span class="badge" :class="svc.installed ? 'badge-success' : 'badge-default'" x-text="svc.installed ? 'Installed' : 'Not installed'"></span>
                                        </td>
                                        <td>
                                            <div class="flex gap-2">
                                                <button x-show="!svc.installed" @click="installService(svc.id)" class="btn-primary text-xs px-3 py-1.5">Install</button>
                                                <button x-show="svc.installed" @click="uninstallService(svc.id)" class="btn-text btn-danger-text text-xs px-3 py-1.5">Uninstall</button>
                                            </div>
                                        </td>
                                    </tr>
                                </template>
                                <tr x-show="installerServices.length === 0">
                                    <td colspan="4" class="text-center py-12 text-slate-400">Loading services...</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                <div x-show="installerBusy" class="text-sm text-indigo-600 flex items-center gap-2"><i class="fa fa-spinner animate-spin"></i> Working, please wait...</div>
                <div x-show="installerMsg" class="text-sm font-medium" :class="installerOk ? 'text-emerald-600' : 'text-red-600'" x-text="installerMsg"></div>
            </div>

            <!-- MySQL Accounts -->
            <div x-cloak x-show="currentPage === 'mysql'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500">Manage MySQL database users</div>
                    <button @click="loadMysqlAccounts()" class="btn-default inline-flex items-center gap-2"><i class="fa fa-sync"></i> Refresh</button>
                </div>
                <div class="fancy-card p-5 space-y-3">
                    <div class="grid grid-cols-1 sm:grid-cols-4 gap-2.5">
                        <input class="input-default" placeholder="username" x-model="mysqlForm.username">
                        <input class="input-default" placeholder="password" type="password" x-model="mysqlForm.password">
                        <input class="input-default" placeholder="host" x-model="mysqlForm.host">
                        <button @click="createMysqlAccount()" class="btn-primary">Create</button>
                    </div>
                    <div x-show="mysqlMsg" class="text-sm font-medium" :class="mysqlOk ? 'text-emerald-600' : 'text-red-600'" x-text="mysqlMsg"></div>
                </div>
                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead><tr><th>User</th><th class="text-right">Actions</th></tr></thead>
                            <tbody>
                                <template x-for="u in mysqlAccounts" :key="u">
                                    <tr>
                                        <td x-text="u" class="font-mono text-sm"></td>
                                        <td class="text-right"><button @click="deleteMysqlAccount(u.split('\t')[0])" class="btn-text btn-danger-text text-xs">Delete</button></td>
                                    </tr>
                                </template>
                                <tr x-show="mysqlAccounts.length === 0">
                                    <td colspan="2" class="text-center py-10 text-slate-400">No accounts / mysql CLI not available</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- PostgreSQL Accounts -->
            <div x-cloak x-show="currentPage === 'postgres'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500">Manage PostgreSQL database users</div>
                    <button @click="loadPostgresAccounts()" class="btn-default inline-flex items-center gap-2"><i class="fa fa-sync"></i> Refresh</button>
                </div>
                <div class="fancy-card p-5 space-y-3">
                    <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5">
                        <input class="input-default" placeholder="username" x-model="pgForm.username">
                        <input class="input-default" placeholder="password" type="password" x-model="pgForm.password">
                        <button @click="createPostgresAccount()" class="btn-primary">Create</button>
                    </div>
                    <div x-show="pgMsg" class="text-sm font-medium" :class="pgOk ? 'text-emerald-600' : 'text-red-600'" x-text="pgMsg"></div>
                </div>
                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead><tr><th>User</th><th class="text-right">Actions</th></tr></thead>
                            <tbody>
                                <template x-for="u in postgresAccounts" :key="u">
                                    <tr>
                                        <td x-text="u" class="font-mono text-sm"></td>
                                        <td class="text-right"><button @click="deletePostgresAccount(u)" class="btn-text btn-danger-text text-xs">Delete</button></td>
                                    </tr>
                                </template>
                                <tr x-show="postgresAccounts.length === 0">
                                    <td colspan="2" class="text-center py-10 text-slate-400">No accounts / psql not available</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- FTP Accounts -->
            <div x-cloak x-show="currentPage === 'ftp'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500">Manage FTP users</div>
                    <button @click="loadFtpAccounts()" class="btn-default inline-flex items-center gap-2"><i class="fa fa-sync"></i> Refresh</button>
                </div>
                <div class="fancy-card p-5 space-y-3">
                    <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5">
                        <input class="input-default" placeholder="username" x-model="ftpForm.username">
                        <input class="input-default" placeholder="password" type="password" x-model="ftpForm.password">
                        <button @click="createFtpAccount()" class="btn-primary">Create</button>
                    </div>
                    <div x-show="ftpMsg" class="text-sm font-medium" :class="ftpOk ? 'text-emerald-600' : 'text-red-600'" x-text="ftpMsg"></div>
                </div>
                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead><tr><th>User</th><th class="text-right">Actions</th></tr></thead>
                            <tbody>
                                <template x-for="u in ftpAccounts" :key="u">
                                    <tr>
                                        <td x-text="u" class="font-mono text-sm"></td>
                                        <td class="text-right"><button @click="deleteFtpAccount(u)" class="btn-text btn-danger-text text-xs">Delete</button></td>
                                    </tr>
                                </template>
                                <tr x-show="ftpAccounts.length === 0">
                                    <td colspan="2" class="text-center py-10 text-slate-400">No accounts</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Users -->
            <div x-cloak x-show="currentPage === 'users'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="flex flex-wrap justify-between items-center gap-3">
                    <div class="text-sm text-slate-500" x-text="`${users.length} user account(s)`"></div>
                    <button @click="showAddUserModal = true" class="btn-primary inline-flex items-center gap-2">
                        <i class="fa fa-plus"></i> Add User
                    </button>
                </div>

                <div class="fancy-card">
                    <div class="table-wrapper">
                        <table>
                            <thead>
                                <tr><th>Username</th><th>Role</th><th>Email</th><th>Status</th><th>Created</th></tr>
                            </thead>
                            <tbody>
                                <template x-for="user in users" :key="user.id">
                                    <tr>
                                        <td class="font-mono text-sm font-semibold text-slate-900" x-text="user.username"></td>
                                        <td><span class="badge badge-primary" x-text="user.role"></span></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="user.email ?? '—'"></td>
                                        <td><span class="badge" :class="user.is_active ? 'badge-success' : 'badge-default'" x-text="user.is_active ? 'Active' : 'Inactive'"></span></td>
                                        <td class="font-mono text-xs text-slate-500" x-text="user.created_at || '--'"></td>
                                    </tr>
                                </template>
                                <tr x-show="users.length === 0">
                                    <td colspan="5" class="text-center py-14 text-slate-400">
                                        <i class="fa fa-users text-3xl mb-2 block opacity-40"></i>
                                        No users found.
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <!-- Settings -->
            <div x-cloak x-show="currentPage === 'settings'"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-3"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 class="space-y-5">
                <div class="fancy-card p-6">
                    <h3 class="ad-card-title">Server Configuration</h3>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
                        <div class="form-group">
                            <label class="form-label">Listen Address</label>
                            <input type="text" class="input-default font-mono" x-model="settingsForm.listen_address">
                        </div>
                        <div class="form-group">
                            <label class="form-label">PHP Version</label>
                            <select class="select-default" x-model="settingsForm.php_version">
                                <option value="8.3">PHP 8.3</option>
                                <option value="8.2">PHP 8.2</option>
                                <option value="8.1">PHP 8.1</option>
                            </select>
                        </div>
                        <div class="form-group">
                            <label class="form-label">Worker Count</label>
                            <input type="number" class="input-default" min="1" x-model="settingsForm.worker_count">
                        </div>
                        <div class="form-group">
                            <label class="form-label">Max Body Size</label>
                            <input type="text" class="input-default" x-model="settingsForm.max_body_size">
                        </div>
                    </div>
                    <div class="pt-2">
                        <button @click="saveSettings()" class="btn-primary inline-flex items-center gap-2">
                            <i class="fa fa-save"></i> Save Settings
                        </button>
                    </div>
                </div>
            </div>

        </div>
    </main>

    <!-- Add / Edit Website Modal -->
    <div x-cloak x-show="showAddWebsiteModal"
         x-transition:enter="transition ease-out duration-200"
         x-transition:enter-start="opacity-0"
         x-transition:enter-end="opacity-100"
         x-transition:leave="transition ease-in duration-150"
         x-transition:leave-start="opacity-100"
         x-transition:leave-end="opacity-0"
         class="modal-overlay"
         @click="closeWebsiteModal()">
        <div class="modal"
             x-show="showAddWebsiteModal"
             x-transition:enter="transition ease-out duration-250"
             x-transition:enter-start="opacity-0 scale-95 translate-y-2"
             x-transition:enter-end="opacity-100 scale-100 translate-y-0"
             x-transition:leave="transition ease-in duration-150"
             x-transition:leave-start="opacity-100 scale-100 translate-y-0"
             x-transition:leave-end="opacity-0 scale-95 translate-y-2"
             @click.stop>
            <button type="button" class="modal-close-btn" @click="closeWebsiteModal()">&times;</button>
            <h3 class="text-xl font-bold text-slate-900 mb-5" x-text="editingWebsiteId ? 'Edit Website' : 'Add Website'"></h3>
            <form @submit.prevent="saveWebsite()">
                <div class="form-group">
                    <label class="form-label">Website Name</label>
                    <input type="text" class="input-default" x-model="newWebsite.name" placeholder="My Website" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domain</label>
                    <input type="text" class="input-default" x-model="newWebsite.domain" placeholder="example.com" :disabled="Boolean(editingWebsiteId)" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Document Root</label>
                    <input type="text" class="input-default font-mono" x-model="newWebsite.document_root" placeholder="/var/www/example.com" required>
                </div>
                <div class="grid grid-cols-2 gap-4">
                    <div class="form-group">
                        <label class="form-label">PHP Version</label>
                        <select class="select-default" x-model="newWebsite.php_version">
                            <option value="8.3">8.3</option>
                            <option value="8.2">8.2</option>
                            <option value="8.1">8.1</option>
                        </select>
                    </div>
                    <div class="form-group">
                        <label class="form-label">Worker Count</label>
                        <input type="number" class="input-default" x-model.number="newWebsite.worker_count" min="1">
                    </div>
                    <div class="form-group">
                        <label class="form-label">Listen Port (optional)</label>
                        <input type="number" class="input-default" x-model.number="newWebsite.listen_port" placeholder="default (80/443)">
                    </div>
                </div>
                <div class="form-group flex items-center gap-2.5">
                    <input id="sslToggle" type="checkbox" class="w-4 h-4 border-slate-300 rounded text-indigo-600 focus:ring-indigo-500" x-model="newWebsite.ssl_enabled">
                    <label for="sslToggle" class="text-slate-600 text-sm cursor-pointer select-none">Enable SSL (Let's Encrypt)</label>
                </div>
                <div class="form-group" x-show="newWebsite.ssl_enabled" x-transition>
                    <label class="form-label">ACME Email</label>
                    <input type="email" class="input-default" x-model="newWebsite.acme_email" placeholder="admin@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-2">
                    <button type="button" @click="closeWebsiteModal()" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6" x-text="editingWebsiteId ? 'Update Website' : 'Add Website'"></button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add SSL Modal -->
    <div x-cloak x-show="showAddSslModal"
         x-transition:enter="transition ease-out duration-200"
         x-transition:enter-start="opacity-0"
         x-transition:enter-end="opacity-100"
         x-transition:leave="transition ease-in duration-150"
         x-transition:leave-start="opacity-100"
         x-transition:leave-end="opacity-0"
         class="modal-overlay"
         @click="showAddSslModal = false">
        <div class="modal"
             x-show="showAddSslModal"
             x-transition:enter="transition ease-out duration-250"
             x-transition:enter-start="opacity-0 scale-95 translate-y-2"
             x-transition:enter-end="opacity-100 scale-100 translate-y-0"
             x-transition:leave="transition ease-in duration-150"
             x-transition:leave-start="opacity-100 scale-100 translate-y-0"
             x-transition:leave-end="opacity-0 scale-95 translate-y-2"
             @click.stop>
            <button type="button" class="modal-close-btn" @click="showAddSslModal = false">&times;</button>
            <h3 class="text-xl font-bold text-slate-900 mb-5">Issue SSL Certificate</h3>
            <form @submit.prevent="issueSsl()">
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-default" x-model="sslForm.email" placeholder="admin@example.com" required>
                </div>
                <div class="form-group">
                    <label class="form-label">Domains (comma separated)</label>
                    <input type="text" class="input-default" x-model="sslForm.domains" placeholder="example.com, www.example.com" required>
                </div>
                <div class="text-xs text-slate-500 mb-4">Uses HTTP-01 challenge. Port 80 must be publicly accessible.</div>
                <div class="flex justify-end gap-3 pt-2">
                    <button type="button" @click="showAddSslModal = false" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6">Issue Certificate</button>
                </div>
            </form>
        </div>
    </div>

    <!-- Add User Modal -->
    <div x-cloak x-show="showAddUserModal"
         x-transition:enter="transition ease-out duration-200"
         x-transition:enter-start="opacity-0"
         x-transition:enter-end="opacity-100"
         x-transition:leave="transition ease-in duration-150"
         x-transition:leave-start="opacity-100"
         x-transition:leave-end="opacity-0"
         class="modal-overlay"
         @click="showAddUserModal = false">
        <div class="modal"
             x-show="showAddUserModal"
             x-transition:enter="transition ease-out duration-250"
             x-transition:enter-start="opacity-0 scale-95 translate-y-2"
             x-transition:enter-end="opacity-100 scale-100 translate-y-0"
             x-transition:leave="transition ease-in duration-150"
             x-transition:leave-start="opacity-100 scale-100 translate-y-0"
             x-transition:leave-end="opacity-0 scale-95 translate-y-2"
             @click.stop>
            <button type="button" class="modal-close-btn" @click="showAddUserModal = false">&times;</button>
            <h3 class="text-xl font-bold text-slate-900 mb-5">Add User</h3>
            <form @submit.prevent="addUser()">
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
                        <option value="admin">admin</option>
                        <option value="viewer">viewer</option>
                    </select>
                </div>
                <div class="form-group">
                    <label class="form-label">Email</label>
                    <input type="email" class="input-default" x-model="newUser.email" placeholder="user@example.com">
                </div>
                <div class="flex justify-end gap-3 pt-2">
                    <button type="button" @click="showAddUserModal = false" class="btn-default px-4">Cancel</button>
                    <button type="submit" class="btn-primary px-6">Add User</button>
                </div>
            </form>
        </div>
    </div>

    <script>
        const PAGE_META = {
            dashboard: { title: 'Dashboard', subtitle: 'Server overview and performance metrics' },
            websites: { title: 'Websites', subtitle: 'Manage your PHP applications and virtual hosts' },
            ssl: { title: 'SSL / TLS Certificates', subtitle: 'Manage Let\'s Encrypt certificates via ACME' },
            workers: { title: 'PHP Workers', subtitle: 'Monitor and control embedded PHP worker processes' },
            files: { title: 'File Manager', subtitle: 'Browse and inspect files on the server' },
            installer: { title: 'Installer', subtitle: 'Install third-party services and tools via OS package manager' },
            mysql: { title: 'MySQL Accounts', subtitle: 'Manage MySQL database users' },
            postgres: { title: 'PostgreSQL Accounts', subtitle: 'Manage PostgreSQL database users' },
            ftp: { title: 'FTP Accounts', subtitle: 'Manage FTP users' },
            users: { title: 'Users', subtitle: 'Manage panel administrators and viewer accounts' },
            settings: { title: 'Settings', subtitle: 'Configure global server runtime options' }
        };

        function blankWebsite() {
            return {
                name: '',
                domain: '',
                document_root: '',
                php_version: '8.3',
                ssl_enabled: false,
                acme_email: '',
                acme_domains: '',
                worker_count: 1,
                listen_port: null
            };
        }

        document.addEventListener('alpine:init', () => {
            Alpine.data('larvaPanel', () => ({
                currentPage: 'dashboard',
                sidebarOpen: false,
                isRefreshing: false,
                lastUpdated: '',
                alert: null,
                alertType: 'info',
                alertTimer: null,
                stats: {
                    total_websites: 0,
                    active_websites: 0,
                    total_workers: 0,
                    running_workers: 0,
                    ssl_certificates: 0,
                    requests_today: 0,
                    cpu_usage: 0,
                    memory_usage: 0,
                    disk_usage: 0
                },
                websites: [],
                sslCerts: [],
                workers: [],
                files: [],
                users: [],
                fileDir: '/',
                installerServices: [],
                installerBusy: false,
                installerMsg: '',
                installerOk: true,
                mysqlAccounts: [],
                mysqlForm: { username: '', password: '', host: 'localhost' },
                mysqlMsg: '',
                mysqlOk: true,
                postgresAccounts: [],
                pgForm: { username: '', password: '' },
                pgMsg: '',
                pgOk: true,
                ftpAccounts: [],
                ftpForm: { username: '', password: '' },
                ftpMsg: '',
                ftpOk: true,
                showAddWebsiteModal: false,
                showAddSslModal: false,
                showAddUserModal: false,
                editingWebsiteId: null,
                newWebsite: blankWebsite(),
                sslForm: { email: '', domains: '' },
                newUser: { username: '', password: '', role: 'admin', email: '' },
                settingsForm: {
                    listen_address: '0.0.0.0:8000',
                    php_version: '8.3',
                    worker_count: 4,
                    max_body_size: '10 MB'
                },

                get pageMeta() {
                    return PAGE_META[this.currentPage] || PAGE_META.dashboard;
                },

                async init() {
                    await this.refreshAll();
                    setInterval(() => this.fetchStats(), 15000);
                },

                navigate(page) {
                    this.currentPage = page;
                    this.sidebarOpen = false;
                    if (page === 'websites') this.fetchWebsites();
                    else if (page === 'ssl') this.fetchSsl();
                    else if (page === 'workers') this.fetchWorkers();
                    else if (page === 'files' && this.files.length === 0) this.listFiles();
                    else if (page === 'installer') this.loadInstaller();
                    else if (page === 'mysql') this.loadMysqlAccounts();
                    else if (page === 'postgres') this.loadPostgresAccounts();
                    else if (page === 'ftp') this.loadFtpAccounts();
                    else if (page === 'users') this.fetchUsers();
                },

                async api(path, method = 'GET', body = null) {
                    const opts = { method, headers: { 'Content-Type': 'application/json' } };
                    if (body) opts.body = JSON.stringify(body);
                    const res = await fetch('/api' + path, opts);
                    const data = await res.json();
                    if (!res.ok || data.success === false) {
                        throw new Error(data.error || 'Request failed');
                    }
                    return data;
                },

                showAlert(msg, type = 'info') {
                    this.alert = msg;
                    this.alertType = type;
                    if (this.alertTimer) clearTimeout(this.alertTimer);
                    this.alertTimer = setTimeout(() => { this.alert = null; }, 4500);
                },

                async refreshAll() {
                    this.isRefreshing = true;
                    try {
                        await Promise.all([
                            this.fetchStats(),
                            this.fetchWebsites(),
                            this.fetchSsl(),
                            this.fetchWorkers(),
                            this.fetchUsers()
                        ]);
                    } finally {
                        setTimeout(() => { this.isRefreshing = false; }, 350);
                    }
                },

                async forceReloadServer() {
                    try {
                        const r = await fetch('/api/server/reload', { method: 'POST' });
                        const d = await r.json();
                        alert(d.success ? 'Server reloaded' : (d.error || 'Failed'));
                        await this.refreshAll();
                    } catch (e) { alert('Error: ' + e.message); }
                },

                async fetchStats() {
                    try {
                        const res = await this.api('/stats');
                        this.stats = res.data || res;
                        this.lastUpdated = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
                    } catch (e) { console.warn(e); }
                },

                async fetchWebsites() {
                    try {
                        const res = await this.api('/websites');
                        this.websites = res.data || [];
                    } catch (e) { console.warn(e); }
                },

                async fetchSsl() {
                    try {
                        const res = await this.api('/ssl');
                        this.sslCerts = res.data || [];
                    } catch (e) { console.warn(e); }
                },

                async fetchWorkers() {
                    try {
                        const res = await this.api('/workers');
                        this.workers = res.data || [];
                    } catch (e) { console.warn(e); }
                },

                async fetchUsers() {
                    try {
                        const res = await this.api('/users');
                        this.users = res.data || [];
                    } catch (e) { console.warn(e); }
                },

                loadInstaller() {
                    this.installerBusy = true;
                    this.installerMsg = '';
                    fetch('/api/installer/services')
                        .then(r => r.json())
                        .then(d => { this.installerServices = d.data || []; this.installerBusy = false; })
                        .catch(e => { this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; });
                },

                installService(id) {
                    this.installerBusy = true;
                    this.installerMsg = '';
                    fetch('/api/installer/install', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ service: id }) })
                        .then(r => r.json())
                        .then(d => { this.installerMsg = d.success ? 'Installed ' + id : (d.error || 'Failed'); this.installerOk = d.success; this.installerBusy = false; this.loadInstaller(); })
                        .catch(e => { this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; });
                },

                uninstallService(id) {
                    this.installerBusy = true;
                    this.installerMsg = '';
                    fetch('/api/installer/uninstall', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ service: id }) })
                        .then(r => r.json())
                        .then(d => { this.installerMsg = d.success ? 'Uninstalled ' + id : (d.error || 'Failed'); this.installerOk = d.success; this.installerBusy = false; this.loadInstaller(); })
                        .catch(e => { this.installerMsg = e.message; this.installerOk = false; this.installerBusy = false; });
                },

                loadMysqlAccounts() {
                    fetch('/api/mysql/accounts').then(r => r.json()).then(d => { this.mysqlAccounts = d.data || []; }).catch(e => console.warn(e));
                },

                createMysqlAccount() {
                    fetch('/api/mysql/accounts', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(this.mysqlForm) })
                        .then(r => r.json())
                        .then(d => { this.mysqlMsg = d.success ? 'Created' : (d.error || 'Failed'); this.mysqlOk = d.success; this.loadMysqlAccounts(); });
                },

                deleteMysqlAccount(u) {
                    fetch('/api/mysql/accounts/' + encodeURIComponent(u), { method: 'DELETE' })
                        .then(r => r.json())
                        .then(d => { this.mysqlMsg = d.success ? 'Deleted' : (d.error || 'Failed'); this.mysqlOk = d.success; this.loadMysqlAccounts(); });
                },

                loadPostgresAccounts() {
                    fetch('/api/postgres/accounts').then(r => r.json()).then(d => { this.postgresAccounts = d.data || []; }).catch(e => console.warn(e));
                },

                createPostgresAccount() {
                    fetch('/api/postgres/accounts', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(this.pgForm) })
                        .then(r => r.json())
                        .then(d => { this.pgMsg = d.success ? 'Created' : (d.error || 'Failed'); this.pgOk = d.success; this.loadPostgresAccounts(); });
                },

                deletePostgresAccount(u) {
                    fetch('/api/postgres/accounts/' + encodeURIComponent(u), { method: 'DELETE' })
                        .then(r => r.json())
                        .then(d => { this.pgMsg = d.success ? 'Deleted' : (d.error || 'Failed'); this.pgOk = d.success; this.loadPostgresAccounts(); });
                },

                loadFtpAccounts() {
                    fetch('/api/ftp/accounts').then(r => r.json()).then(d => { this.ftpAccounts = d.data || []; }).catch(e => console.warn(e));
                },

                createFtpAccount() {
                    fetch('/api/ftp/accounts', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(this.ftpForm) })
                        .then(r => r.json())
                        .then(d => { this.ftpMsg = d.success ? 'Created' : (d.error || 'Failed'); this.ftpOk = d.success; this.loadFtpAccounts(); });
                },

                deleteFtpAccount(u) {
                    fetch('/api/ftp/accounts/' + encodeURIComponent(u), { method: 'DELETE' })
                        .then(r => r.json())
                        .then(d => { this.ftpMsg = d.success ? 'Deleted' : (d.error || 'Failed'); this.ftpOk = d.success; this.loadFtpAccounts(); });
                },

                openNewWebsiteModal() {
                    this.editingWebsiteId = null;
                    this.newWebsite = blankWebsite();
                    this.showAddWebsiteModal = true;
                },

                editWebsite(site) {
                    this.editingWebsiteId = site.id;
                    this.newWebsite = { ...site };
                    this.showAddWebsiteModal = true;
                },

                closeWebsiteModal() {
                    this.showAddWebsiteModal = false;
                    this.editingWebsiteId = null;
                    this.newWebsite = blankWebsite();
                },

                async saveWebsite() {
                    try {
                        if (this.editingWebsiteId) {
                            await this.api('/websites/' + this.editingWebsiteId, 'PUT', this.newWebsite);
                            this.showAlert('Website updated successfully', 'success');
                        } else {
                            await this.api('/websites', 'POST', this.newWebsite);
                            this.showAlert('Website added successfully', 'success');
                        }
                        this.closeWebsiteModal();
                        await Promise.all([this.fetchWebsites(), this.fetchStats()]);
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async deleteWebsite(id) {
                    if (!confirm('Delete this website?')) return;
                    try {
                        await this.api('/websites/' + id, 'DELETE');
                        this.showAlert('Website deleted', 'success');
                        await Promise.all([this.fetchWebsites(), this.fetchStats()]);
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async issueSsl() {
                    try {
                        const domains = this.sslForm.domains.split(',').map(d => d.trim()).filter(Boolean);
                        await this.api('/ssl/issue', 'POST', { email: this.sslForm.email, domains });
                        this.showAlert('SSL certificate issued', 'success');
                        this.showAddSslModal = false;
                        this.sslForm = { email: '', domains: '' };
                        await Promise.all([this.fetchSsl(), this.fetchStats()]);
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async renewCert(id) {
                    try {
                        await this.api('/ssl/' + id + '/renew', 'POST');
                        this.showAlert('Certificate renewal triggered', 'success');
                        await this.fetchSsl();
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async deleteCert(id) {
                    if (!confirm('Delete this certificate?')) return;
                    try {
                        await this.api('/ssl/' + id, 'DELETE');
                        this.showAlert('Certificate deleted', 'success');
                        await Promise.all([this.fetchSsl(), this.fetchStats()]);
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async addUser() {
                    try {
                        await this.api('/users', 'POST', this.newUser);
                        this.showAlert('User added successfully', 'success');
                        this.showAddUserModal = false;
                        this.newUser = { username: '', password: '', role: 'admin', email: '' };
                        await this.fetchUsers();
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async listFiles() {
                    try {
                        const res = await this.api('/files?dir=' + encodeURIComponent(this.fileDir));
                        this.files = res.data || [];
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async inspectFile(path) {
                    try {
                        const res = await this.api('/files?path=' + encodeURIComponent(path));
                        this.showAlert(res.data?.content || `Loaded ${path}`, 'info');
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async restartWorker(id) {
                    try {
                        await this.api('/workers/' + id + '/restart', 'POST');
                        this.showAlert('Worker restarted', 'success');
                        await this.fetchWorkers();
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async stopWorker(id) {
                    try {
                        await this.api('/workers/' + id + '/stop', 'POST');
                        this.showAlert('Worker stopped', 'success');
                        await this.fetchWorkers();
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                async saveSettings() {
                    try {
                        await this.api('/settings', 'PUT', { value: JSON.stringify(this.settingsForm) });
                        this.showAlert('Settings saved', 'success');
                    } catch (e) { this.showAlert(e.message, 'error'); }
                },

                closeModal() {
                    this.showAddWebsiteModal = false;
                    this.showAddSslModal = false;
                    this.showAddUserModal = false;
                    this.sidebarOpen = false;
                }
            }));
        });
    </script>
</body>
</html>"##
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::panel_html;

    #[test]
    fn panel_html_uses_fixed_layout_alpine_v3_and_mascot_logo() {
        let html = panel_html();
        assert!(html.contains("position: fixed;"));
        assert!(html.contains("min-height: 100vh;"));
        assert!(html.contains("Alpine.data('larvaPanel'"));
        assert!(html.contains("<template x-for=\"site in websites\""));
        assert!(html.contains("LarvaPHP Mascot Logo"));
        assert!(html.contains("rel=\"icon\" type=\"image/svg+xml\""));
        assert!(html.contains("navigate('installer')"));
        assert!(html.contains("navigate('mysql')"));
        assert!(html.contains("navigate('postgres')"));
        assert!(html.contains("navigate('ftp')"));
        assert!(html.contains("@keyframes fadeSlideUp"));
        assert!(html.contains("prefers-reduced-motion: reduce"));
        assert!(!html.contains(".__x"));
        assert!(!html.contains("min-h-screen: 100vh"));
    }
}
