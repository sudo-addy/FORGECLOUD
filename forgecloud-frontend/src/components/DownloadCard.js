import { motion } from "framer-motion";
import { DownloadCloud, CheckCircle, AlertCircle } from "lucide-react";

export function DownloadCard({
  downloadFileId,
  setDownloadFileId,
  isDownloading,
  downloadStatus,
  handleDownload
}) {
  return (
    <motion.div 
      initial={{ opacity: 0, y: 40 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.8, delay: 0.2, ease: "easeOut" }}
      className="bg-gradient-to-b from-white/[0.08] to-transparent border-t border-white/20 border-x-0 border-b-0 backdrop-blur-xl shadow-2xl rounded-2xl p-8 flex flex-col items-center w-full md:w-96 gap-6 relative"
    >
      <div className="absolute inset-0 bg-gradient-to-b from-white/[0.02] to-transparent rounded-2xl pointer-events-none" />
      <div className="flex items-center gap-3 relative z-10">
        <DownloadCloud className="text-indigo-400" size={28} />
        <h2 className="text-xl font-light tracking-[0.2em] text-white">DOWNLOAD</h2>
      </div>
      
      <form onSubmit={handleDownload} className="flex flex-col items-center gap-6 w-full relative z-10">
        <div className="w-full relative">
          <input 
            type="text" 
            placeholder="PASTE UUID..."
            value={downloadFileId}
            onChange={(e) => setDownloadFileId(e.target.value)}
            className="w-full bg-black/40 border border-white/10 text-white rounded-xl px-4 py-4 focus:outline-none focus:border-indigo-400/50 text-center text-xs tracking-widest placeholder:text-zinc-600 transition-colors"
          />
        </div>
        
        <button 
          type="submit" 
          disabled={isDownloading || !downloadFileId.trim()}
          className="w-full py-3 bg-white/10 border border-white/10 text-white text-sm tracking-[0.1em] rounded hover:bg-white/20 hover:shadow-[0_0_20px_rgba(255,255,255,0.2)] disabled:opacity-30 disabled:cursor-not-allowed transition-all"
        >
          {isDownloading ? "DECRYPTING..." : "INITIATE TRANSFER"}
        </button>
      </form>

      {downloadStatus && (
        <p className="text-xs text-indigo-300 tracking-wider text-center mt-2 flex items-center justify-center gap-2 relative z-10">
          {downloadStatus.includes("Error") ? <AlertCircle size={14} className="text-red-400" /> : <CheckCircle size={14} />}
          {downloadStatus}
        </p>
      )}
    </motion.div>
  );
}
