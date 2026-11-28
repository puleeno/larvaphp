local M = _G.shorten
larva.route("GET", "/dashboard", function(req)
    local rows = ""
    for code, url in pairs(M.links) do
        rows = rows .. "<tr class='border-b border-slate-100'><td class='px-4 py-2 font-mono text-indigo-600'>" .. code .. "</td><td class='px-4 py-2 text-slate-600 truncate max-w-xs'>" .. url .. "</td></tr>"
    end
    local html = [[<!DOCTYPE html><html><head><script src="https://cdn.tailwindcss.com"></script></head><body class="bg-slate-50 min-h-screen p-8">
    <div class="max-w-3xl mx-auto">
      <h1 class="text-3xl font-bold text-slate-900 mb-6">Shorten Links</h1>
      <div class="bg-white rounded-2xl shadow-sm border border-slate-200 p-6 mb-6">
        <form method="POST" action="/api/shorten" class="flex gap-3">
          <input name="url" placeholder="https://example.com/long-url" class="flex-1 border border-slate-300 rounded-lg px-4 py-2 focus:outline-none focus:ring-2 focus:ring-indigo-500">
          <button class="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-2 rounded-lg font-medium">Shorten</button>
        </form>
      </div>
      <div class="bg-white rounded-2xl shadow-sm border border-slate-200 overflow-hidden">
        <table class="w-full"><thead class="bg-slate-50"><tr><th class="px-4 py-3 text-left text-xs font-semibold text-slate-500 uppercase">Code</th><th class="px-4 py-3 text-left text-xs font-semibold text-slate-500 uppercase">URL</th></tr></thead>
        <tbody>]] .. rows .. [[</tbody></table>
      </div>
    </div></body></html>]]
    return { status = 200, body = html }
end)
larva.route("GET", "/", function(req)
    return { status = 302, headers = { Location = "/dashboard" } }
end)
