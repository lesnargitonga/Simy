import { RatchetTest } from "./RatchetTest";

function App() {
  return <RatchetTest />;
}

export default App;
    <div className="flex h-screen bg-slate-50 dark:bg-slate-900 transition-colors duration-300">
      <div className="flex-1 flex flex-col w-full max-w-4xl mx-auto h-full px-4 border-x border-slate-200 dark:border-slate-800">
        <header className="flex items-center justify-between py-4 border-b border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-2">
            <h1 className="text-xl font-bold text-slate-900 dark:text-white">Simy</h1>
            {loginType === 'admin' && <span className="bg-red-500/10 text-red-500 text-xs px-2 py-1 rounded-full font-medium border border-red-500/20">ADMIN</span>}
          </div>
          <div className="flex items-center gap-4">
             <button 
              onClick={() => setDarkMode(!darkMode)}
              className="p-2 rounded-full hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-300 transition-colors"
            >
              {darkMode ? <Sun size={20} /> : <Moon size={20} />}
            </button>
            <div className="flex items-center gap-2 text-sm bg-slate-100 dark:bg-slate-800 px-3 py-1.5 rounded-full border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300">
              <UserIcon size={14} />
              <span>{username}</span>
            </div>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto py-6 space-y-6">
          {/* Mock Feed Post */}
          <div className="bg-white dark:bg-slate-800 rounded-xl p-4 shadow-sm border border-slate-200 dark:border-slate-700 relative group">
            
            {loginType === 'admin' && (
               <div className="absolute top-4 right-4 hidden group-hover:flex gap-2">
                 <button className="text-xs bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 px-2 py-1 rounded border border-red-200 dark:border-red-800 hover:bg-red-200 dark:hover:bg-red-900/50 transition">Delete Post</button>
                 <button className="text-xs bg-orange-100 dark:bg-orange-900/30 text-orange-600 dark:text-orange-400 px-2 py-1 rounded border border-orange-200 dark:border-orange-800 hover:bg-orange-200 dark:hover:bg-orange-900/50 transition">Ban User</button>
               </div>
            )}

            <div className="flex items-center gap-3 mb-3">
              <div className="w-8 h-8 rounded-full bg-gradient-to-tr from-purple-500 to-blue-500 flex items-center justify-center text-white font-bold text-xs">
                S
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <div className="text-sm font-medium text-slate-900 dark:text-white">SystemUser_99</div>
                  <div className="text-xs text-slate-500 font-mono">0xA1F...90C</div>
                </div>
                <div className="text-xs text-slate-500">Just now • E2E Encrypted</div>
              </div>
            </div>
            <p className="text-slate-700 dark:text-slate-300">Here's a highly confidential image post for the system feed!</p>
            <div className="mt-3 w-full h-48 bg-slate-200 dark:bg-slate-700 rounded-lg flex items-center justify-center border border-slate-300 dark:border-slate-600 text-slate-400">
               [ Encrypted Image Blob ]
            </div>
          </div>
        </div>

        {loginType === 'admin' && (
          <div className="p-4 border-t border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/50">
             <h2 className="text-sm font-bold text-slate-800 dark:text-slate-200 mb-2">Omnipotent Admin Controls</h2>
             <div className="flex gap-2">
               <button className="flex-1 bg-red-600 hover:bg-red-700 text-white text-sm py-2 rounded shadow transition">Wipe Feed Database</button>
               <button className="flex-1 bg-slate-800 hover:bg-slate-900 dark:bg-black dark:hover:bg-slate-950 text-white text-sm py-2 rounded shadow border border-slate-700 transition">Revoke All Sessions</button>
               <button className="flex-1 bg-blue-600 hover:bg-blue-700 text-white text-sm py-2 rounded shadow transition">Broadcast Global Alert</button>
             </div>
          </div>
        )}

      </div>
    </div>
  );
}

export default App;
