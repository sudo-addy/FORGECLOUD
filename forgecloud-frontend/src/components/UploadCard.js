import { motion } from "framer-motion";
import { UploadCloud, File as FileIcon, CheckCircle, AlertCircle } from "lucide-react";

export function UploadCard({ 
  selectedFile, 
  setSelectedFile, 
  isUploading, 
  uploadProgress, 
  uploadStatus, 
  fileId, 
  handleUpload 
}) {
  return (
    <motion.div 
      initial={{ opacity: 0, y: 40 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.8, delay: 0.1, ease: "easeOut" }}
      className="bg-gradient-to-b from-white/[0.08] to-transparent border-t border-white/20 border-x-0 border-b-0 backdrop-blur-xl shadow-2xl rounded-2xl p-8 flex flex-col items-center w-full md:w-96 gap-6 relative group"
    >
      <div className="absolute inset-0 bg-gradient-to-b from-white/[0.02] to-transparent rounded-2xl pointer-events-none" />
      <div className="flex items-center gap-3 relative z-10">
        <UploadCloud className="text-indigo-400" size={28} />
        <h2 className="text-xl font-light tracking-[0.2em] text-white">UPLOAD</h2>
      </div>
      
      <form onSubmit={handleUpload} className="flex flex-col items-center gap-6 w-full relative z-10">
        <label className="w-full relative cursor-pointer group/upload">
          <input 
            type="file" 
            onChange={(e) => setSelectedFile(e.target.files[0])}
            className="hidden"
          />
          <div className="w-full border border-dashed border-white/20 rounded-xl p-6 flex flex-col items-center justify-center gap-2 bg-black/20 group-hover/upload:border-indigo-400/50 group-hover/upload:bg-indigo-900/10 transition-all">
            <FileIcon className={selectedFile ? "text-indigo-400" : "text-zinc-600"} size={24} />
            <span className="text-xs tracking-wider text-zinc-400 text-center truncate w-full px-2">
              {selectedFile ? selectedFile.name : "SELECT FILE"}
            </span>
          </div>
        </label>
        
        {isUploading ? (
          <div className="w-full flex flex-col gap-2">
            <div className="w-full bg-black/40 border border-white/5 rounded-lg h-10 overflow-hidden relative shadow-inner">
              <motion.div 
                className="absolute top-0 left-0 h-full bg-indigo-500/80 shadow-[0_0_15px_rgba(99,102,241,0.5)]"
                initial={{ width: 0 }}
                animate={{ width: `${uploadProgress}%` }}
                transition={{ ease: "linear" }}
              />
              <div className="absolute inset-0 flex items-center justify-center">
                <span className="text-xs font-mono text-white tracking-[0.2em] font-bold z-10 drop-shadow-[0_2px_2px_rgba(0,0,0,1)]">
                  {uploadProgress}%
                </span>
              </div>
            </div>
          </div>
        ) : (
          <button 
            type="submit" 
            disabled={!selectedFile}
            className="w-full py-3 bg-white/10 border border-white/10 text-white text-sm tracking-[0.1em] rounded hover:bg-white/20 hover:shadow-[0_0_20px_rgba(255,255,255,0.2)] disabled:opacity-30 disabled:cursor-not-allowed transition-all"
          >
            EXECUTE SECURE UPLOAD
          </button>
        )}
      </form>

      {(uploadStatus || fileId) && (
        <div className="w-full mt-2 relative z-10">
          {uploadStatus && (
            <p className="text-xs text-indigo-300 tracking-wider text-center mb-4 flex items-center justify-center gap-2">
              {uploadStatus.includes("Error") ? <AlertCircle size={14} className="text-red-400" /> : <CheckCircle size={14} />}
              {uploadStatus}
            </p>
          )}
          
          {fileId && (
            <div className="w-full p-4 bg-black/40 border border-white/5 rounded-lg text-center overflow-hidden backdrop-blur-md">
              <p className="text-[10px] text-zinc-500 mb-2 tracking-[0.3em]">SECURE FILE ID</p>
              <p className="text-xs font-mono text-white break-all select-all">{fileId}</p>
            </div>
          )}
        </div>
      )}
    </motion.div>
  );
}
