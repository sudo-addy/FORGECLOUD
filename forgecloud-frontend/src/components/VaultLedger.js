import React, { useState } from "react";
import { motion } from "framer-motion";
import { Folder as FolderIcon, ChevronRight, Eye, CornerUpRight, DownloadCloud, Trash2, Link as LinkIcon } from "lucide-react";
import ShareModal from "./ShareModal";

export function VaultLedger({
  isAuthorized,
  vaultFiles,
  folders,
  isFetchingFiles,
  newFolderName,
  setNewFolderName,
  isCreatingFolder,
  handleCreateFolder,
  breadcrumbs,
  currentFolderId,
  setCurrentFolderId,
  setBreadcrumbs,
  handlePreview,
  handleMoveFile,
  handleDownload,
  handleDelete,
  isDownloading
}) {
  const [activeShareFile, setActiveShareFile] = useState(null);

  return (
    <motion.div 
      initial={{ opacity: 0, y: 40 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.8, delay: 0.3, ease: "easeOut" }}
      className="w-full max-w-4xl mt-4 flex flex-col gap-4 relative z-10"
    >
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between pl-2 gap-4">
        <h2 className="text-xs font-light tracking-[0.4em] text-indigo-400 shadow-indigo-500/20 drop-shadow-md">ENCRYPTED LEDGER</h2>
        {isAuthorized && (
          <form onSubmit={handleCreateFolder} className="flex items-center gap-2 w-full sm:w-auto">
            <input 
              type="text"
              value={newFolderName}
              onChange={(e) => setNewFolderName(e.target.value)}
              placeholder="NEW FOLDER NAME..."
              className="bg-white/[0.05] border border-white/10 text-white rounded px-3 py-1.5 focus:outline-none focus:border-amber-400/50 text-xs tracking-widest placeholder:text-zinc-600 transition-colors w-full sm:w-48"
            />
            <button 
              type="submit"
              disabled={isCreatingFolder || !newFolderName.trim()}
              className="p-1.5 bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 rounded transition-colors disabled:opacity-30"
            >
              <FolderIcon size={16} />
            </button>
          </form>
        )}
      </div>

      {/* Breadcrumb Navigation */}
      <div className="flex items-center gap-2 text-xs font-mono text-zinc-400 pl-2 bg-white/[0.02] py-2 px-3 rounded-lg border border-white/[0.05]">
        {breadcrumbs.map((crumb, idx) => (
          <div key={crumb.id || "root"} className="flex items-center gap-2">
            <button 
              onClick={() => {
                setCurrentFolderId(crumb.id);
                setBreadcrumbs(breadcrumbs.slice(0, idx + 1));
              }}
              className={`hover:text-indigo-400 transition-colors tracking-widest uppercase ${currentFolderId === crumb.id ? "text-indigo-300 font-bold" : ""}`}
            >
              {crumb.name}
            </button>
            {idx < breadcrumbs.length - 1 && <ChevronRight size={14} className="text-zinc-600" />}
          </div>
        ))}
      </div>
      
      {isFetchingFiles && vaultFiles.length === 0 && folders.length === 0 ? (
        <p className="text-zinc-500 text-xs tracking-widest pl-2">SYNCING WITH VAULT...</p>
      ) : vaultFiles.length === 0 && folders.length === 0 ? (
        <p className="text-zinc-500 text-xs tracking-widest pl-2">NO FILES OR FOLDERS DETECTED.</p>
      ) : (
        <div className="flex flex-col gap-3">
          {/* Render Folders First */}
          {folders.map((folder, i) => (
            <motion.div 
              key={folder.id}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: i * 0.05, ease: "easeOut" }}
              onClick={() => {
                setCurrentFolderId(folder.id);
                setBreadcrumbs([...breadcrumbs, { id: folder.id, name: folder.name }]);
              }}
              className="bg-gradient-to-r from-amber-900/[0.1] to-transparent border-l-[3px] border-amber-500/50 border-t border-white/5 border-r border-b backdrop-blur-md rounded-r-lg rounded-bl-lg p-4 flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 cursor-pointer group hover:bg-amber-900/[0.15] transition-colors"
            >
              <div className="flex items-center gap-3">
                <FolderIcon size={20} className="text-amber-500/80 group-hover:text-amber-400 transition-colors" />
                <span className="font-semibold text-white tracking-wide">{folder.name}</span>
              </div>
            </motion.div>
          ))}

          {/* Render Files */}
          {vaultFiles.map((file, i) => (
            <motion.div 
              key={file.id}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.5, delay: 0.4 + (i * 0.05), ease: "easeOut" }}
              className="bg-gradient-to-r from-white/[0.05] to-transparent border-t border-white/10 backdrop-blur-md rounded-lg p-5 flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 group hover:bg-white/[0.08] transition-colors"
            >
              <div className="flex flex-col gap-1">
                <span className="font-semibold text-white tracking-wide">{file.name}</span>
                <span className="text-xs text-zinc-500 tracking-wider">
                  {new Date(file.created_at).toLocaleString()} &nbsp;•&nbsp; {(file.total_size / 1024).toFixed(2)} KB
                </span>
              </div>
              
              <div className="flex items-center gap-4 w-full sm:w-auto justify-between sm:justify-end">
                <span className="text-xs font-mono text-zinc-600 tracking-widest truncate max-w-[150px] sm:max-w-none">{file.id}</span>
                <div className="flex items-center gap-2">
                  <button 
                    onClick={() => handlePreview(file)}
                    className="p-2 bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 hover:text-blue-300 rounded transition-colors"
                    title="Preview File"
                  >
                    <Eye size={18} />
                  </button>
                  <button 
                    onClick={() => handleMoveFile(file.id)}
                    className="p-2 bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 hover:text-emerald-300 rounded transition-colors"
                    title="Move File"
                  >
                    <CornerUpRight size={18} />
                  </button>
                  <button 
                    onClick={() => handleDownload(null, file.id)}
                    disabled={isDownloading}
                    className="p-2 bg-indigo-500/10 text-indigo-400 hover:bg-indigo-500/20 hover:text-indigo-300 rounded transition-colors disabled:opacity-50"
                    title="Download File"
                  >
                    <DownloadCloud size={18} />
                  </button>
                  <button 
                    onClick={() => setActiveShareFile(file)}
                    className="p-2 bg-indigo-500/10 text-indigo-400 hover:bg-indigo-500/20 hover:text-indigo-300 rounded transition-colors"
                    title="Share File"
                  >
                    <LinkIcon size={18} />
                  </button>
                  <button 
                    onClick={() => handleDelete(file.id)}
                    className="p-2 bg-red-500/10 text-red-400 hover:bg-red-500/20 hover:text-red-300 rounded transition-colors"
                    title="Delete File"
                  >
                    <Trash2 size={18} />
                  </button>
                </div>
              </div>
            </motion.div>
          ))}
        </div>
      )}

      {activeShareFile && (
        <ShareModal 
          file={activeShareFile} 
          onClose={() => setActiveShareFile(null)} 
        />
      )}
    </motion.div>
  );
}
