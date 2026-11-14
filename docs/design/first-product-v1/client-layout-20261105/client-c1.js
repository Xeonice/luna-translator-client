/* C1 candidate only. No product or approval mutation. Query dimensions choose the
   native client, not a cropped desktop. Existing source owns all interactions. */
(()=>{const q=new URLSearchParams(location.search);document.documentElement.dataset.clientCandidate='c1';document.documentElement.dataset.clientRole=q.get('scenario')==='settings'?'settings':'main';document.documentElement.dataset.clientSettings=q.get('settingsSize')==='native'?'native':'flow';})();

// Mirror the source-owned open/close state when navigating within the prototype.
document.addEventListener('click',()=>setTimeout(()=>{const state=window.LunaDesign?.state;if(state)document.documentElement.dataset.clientRole=state.windows.settings?'settings':'main';},0),true);
