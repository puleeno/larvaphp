local M = _G.shorten
larva.route("GET", "/dashboard", function(req)
    local html = [[<!DOCTYPE html><html><head><script src="https://cdn.tailwindcss.com"></script><script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script></head><body class="bg-slate-50 min-h-screen" x-data="{page:'list'}">
    <div class="max-w-4xl mx-auto p-8">
      <h1 class="text-3xl font-bold text-slate-900 mb-6">Shorten Links</h1>
      <nav class="flex gap-4 mb-6 text-sm">
        <a href="#list" @click="page='list'" class="text-indigo-600 font-medium">Links</a>
        <a href="#create" @click="page='create'" class="text-indigo-600 font-medium">Create</a>
      </nav>
      <div x-show="page==='list'" class="bg-white rounded-2xl shadow-sm border border-slate-200 overflow-hidden">
        <table class="w-full"><thead class="bg-slate-50"><tr><th class="px-4 py-3 text-left text-xs font-semibold text-slate-500 uppercase">Code</th><th class="px-4 py-3 text-left text-xs font-semibold text-slate-500 uppercase">URL</th></tr></thead>
        <tbody><tr><td class="px-4 py-2 font-mono text-indigo-600">demo</td><td class="px-4 py-2 text-slate-600">https://example.com</td></tr></tbody></table>
      </div>
      <div x-show="page==='create'" class="bg-white rounded-2xl shadow-sm border border-slate-200 p-6">
        <form method="POST" action="/api/shorten" class="flex gap-3">
          <input name="url" placeholder="https://example.com/long-url" class="flex-1 border border-slate-300 rounded-lg px-4 py-2">
          <button class="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-2 rounded-lg font-medium">Shorten</button>
        </form>
      </div>
    </div></body></html>]]
    return { status = 200, body = html }
end)
larva.route("GET", "/", function(req)
    return { status = 302, headers = { Location = "/dashboard" } }
end)
