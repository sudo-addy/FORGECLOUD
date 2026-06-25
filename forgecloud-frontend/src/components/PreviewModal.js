import { motion, AnimatePresence } from "framer-motion";
import { File as FileIcon, X } from "lucide-react";

export function PreviewModal({ 
  previewFile, 
  previewBlobUrl, 
  previewTextContent, 
  isPreviewLoading, 
  closePreview 
}) {
  return (
    <AnimatePresence>
      {previewFile && (
        <div 
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-xl p-4 sm:p-8"
          onClick={closePreview}
        >
          <motion.div 
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            onClick={(e) => e.stopPropagation()}
            className="bg-zinc-900 border border-white/10 rounded-2xl w-full max-w-5xl max-h-full flex flex-col overflow-hidden shadow-2xl relative"
          >
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-white/10 bg-black/40">
              <div className="flex items-center gap-3 overflow-hidden">
                <FileIcon className="text-indigo-400 shrink-0" size={20} />
                <h3 className="text-white font-medium tracking-wide truncate">{previewFile.name}</h3>
              </div>
              <button 
                onClick={closePreview}
                className="p-2 text-zinc-400 hover:text-white hover:bg-white/10 rounded-full transition-colors shrink-0"
              >
                <X size={20} />
              </button>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-auto p-4 flex items-center justify-center min-h-[50vh] bg-black/20">
              {isPreviewLoading ? (
                <div className="flex flex-col items-center gap-4">
                  <div className="w-8 h-8 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
                  <p className="text-xs text-indigo-300 tracking-widest font-mono uppercase">DECRYPTING PAYLOAD...</p>
                </div>
              ) : (
                <>
                  {previewFile.mime_type?.startsWith("image/") && previewBlobUrl && (
                    <img src={previewBlobUrl} alt={previewFile.name} className="max-w-full max-h-[70vh] object-contain rounded" />
                  )}
                  {previewFile.mime_type?.startsWith("video/") && previewBlobUrl && (
                    <video src={previewBlobUrl} controls className="max-w-full max-h-[70vh] rounded outline-none" />
                  )}
                  {previewFile.mime_type?.startsWith("audio/") && previewBlobUrl && (
                    <audio src={previewBlobUrl} controls className="w-full max-w-md outline-none" />
                  )}
                  {previewFile.mime_type === "application/pdf" && previewBlobUrl && (
                    <iframe src={previewBlobUrl} className="w-full h-[70vh] rounded border border-white/5 bg-zinc-800" />
                  )}
                  {(previewFile.mime_type?.startsWith("text/") || previewFile.mime_type === "application/json") && previewTextContent !== null && (
                    <pre className="w-full h-full max-h-[70vh] overflow-auto text-xs font-mono text-zinc-300 whitespace-pre-wrap break-words text-left p-4 bg-black/30 rounded border border-white/5">
                      {previewTextContent}
                    </pre>
                  )}
                </>
              )}
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
