import React, { useState, useEffect } from "react";
import { Link, Copy, Trash2, X, Lock, Key, Clock, Download } from "lucide-react";
import { CONFIG } from "../lib/config";
import { API_ROUTES } from "../constants/routes";
import { getAuthHeaders } from "../lib/api";

export default function ShareModal({ file, onClose }) {
  const [shares, setShares] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  
  // Form State
  const [password, setPassword] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [maxDownloads, setMaxDownloads] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    fetchShares();
  }, [file]);

  const fetchShares = async () => {
    setLoading(true);
    try {
      const res = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.SHARES_LIST(file.id)}`, {
        headers: getAuthHeaders(),
      });
      if (!res.ok) throw new Error("Failed to fetch shares");
      const data = await res.json();
      setShares(data);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateShare = async (e) => {
    e.preventDefault();
    setCreating(true);
    setError(null);
    try {
      const payload = {};
      if (password) payload.password = password;
      if (maxDownloads) payload.max_downloads = parseInt(maxDownloads, 10);
      if (expiresAt) {
        // Assume date string, convert to ISO
        payload.expires_at = new Date(expiresAt).toISOString();
      }

      const res = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.SHARES_CREATE(file.id)}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...getAuthHeaders()
        },
        body: JSON.stringify(payload),
      });

      if (!res.ok) throw new Error("Failed to create share link");
      
      const newShare = await res.json();
      setShares([newShare, ...shares]);
      
      // Reset form
      setPassword("");
      setExpiresAt("");
      setMaxDownloads("");
    } catch (err) {
      setError(err.message);
    } finally {
      setCreating(false);
    }
  };

  const handleRevokeShare = async (shareId) => {
    try {
      const res = await fetch(`${CONFIG.API_BASE_URL}${API_ROUTES.SHARES_DELETE(shareId)}`, {
        method: "DELETE",
        headers: getAuthHeaders(),
      });
      if (!res.ok) throw new Error("Failed to revoke share link");
      setShares(shares.filter(s => s.id !== shareId));
    } catch (err) {
      setError(err.message);
    }
  };

  const copyToClipboard = (token) => {
    const url = `${window.location.origin}/s/${token}`;
    navigator.clipboard.writeText(url);
    // Could add toast here
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div className="w-full max-w-2xl bg-zinc-900 border border-zinc-800 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        
        {/* Header */}
        <div className="p-6 border-b border-zinc-800 flex items-center justify-between">
          <div>
            <h3 className="text-xl font-bold text-zinc-100 flex items-center gap-2">
              <Link className="w-5 h-5 text-indigo-400" />
              Share File
            </h3>
            <p className="text-sm text-zinc-400 mt-1 truncate max-w-md">
              {file.name}
            </p>
          </div>
          <button 
            onClick={onClose}
            className="p-2 text-zinc-400 hover:text-white hover:bg-zinc-800 rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-8">
          
          {/* Create Form */}
          <form onSubmit={handleCreateShare} className="space-y-4">
            <h4 className="text-sm font-medium text-zinc-300 uppercase tracking-wider mb-2">Create New Link</h4>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm text-zinc-400 flex items-center gap-1">
                  <Key className="w-4 h-4" /> Password (Optional)
                </label>
                <input 
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-100 focus:outline-none focus:border-indigo-500"
                  placeholder="Leave blank for public"
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm text-zinc-400 flex items-center gap-1">
                  <Download className="w-4 h-4" /> Max Downloads (Optional)
                </label>
                <input 
                  type="number"
                  min="1"
                  value={maxDownloads}
                  onChange={(e) => setMaxDownloads(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-100 focus:outline-none focus:border-indigo-500"
                  placeholder="Unlimited"
                />
              </div>

              <div className="space-y-2 md:col-span-2">
                <label className="text-sm text-zinc-400 flex items-center gap-1">
                  <Clock className="w-4 h-4" /> Expiration Date & Time (Optional)
                </label>
                <input 
                  type="datetime-local"
                  value={expiresAt}
                  onChange={(e) => setExpiresAt(e.target.value)}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-2 text-zinc-100 focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            {error && (
              <div className="p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-sm">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={creating}
              className="w-full bg-indigo-600 hover:bg-indigo-700 text-white font-medium py-2 px-4 rounded-lg transition-colors disabled:opacity-50"
            >
              {creating ? "Generating Link..." : "Generate Share Link"}
            </button>
          </form>

          {/* Existing Shares */}
          <div>
            <h4 className="text-sm font-medium text-zinc-300 uppercase tracking-wider mb-4 border-t border-zinc-800 pt-6">
              Active Share Links
            </h4>
            
            {loading ? (
              <p className="text-zinc-500 text-sm text-center py-4">Loading shares...</p>
            ) : shares.length === 0 ? (
              <p className="text-zinc-500 text-sm text-center py-4">No active share links.</p>
            ) : (
              <div className="space-y-3">
                {shares.map(share => (
                  <div key={share.id} className="bg-zinc-950 border border-zinc-800 rounded-lg p-4 flex flex-col gap-3">
                    <div className="flex items-start justify-between gap-4">
                      <div className="flex-1 truncate">
                        <code className="text-xs text-indigo-400 bg-indigo-400/10 px-2 py-1 rounded select-all">
                          {typeof window !== 'undefined' ? window.location.origin : ''}/s/{share.token}
                        </code>
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        <button
                          onClick={(e) => {
                            e.preventDefault();
                            copyToClipboard(share.token);
                          }}
                          className="p-1.5 text-zinc-400 hover:text-indigo-400 hover:bg-zinc-800 rounded transition-colors"
                          title="Copy Link"
                        >
                          <Copy className="w-4 h-4" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.preventDefault();
                            handleRevokeShare(share.id);
                          }}
                          className="p-1.5 text-zinc-400 hover:text-red-400 hover:bg-zinc-800 rounded transition-colors"
                          title="Revoke Link"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                    
                    <div className="flex flex-wrap items-center gap-4 text-xs text-zinc-500">
                      {share.has_password && (
                        <span className="flex items-center gap-1 text-emerald-400">
                          <Lock className="w-3 h-3" /> Password Protected
                        </span>
                      )}
                      {share.max_downloads && (
                        <span className="flex items-center gap-1">
                          <Download className="w-3 h-3" /> {share.download_count} / {share.max_downloads} dl
                        </span>
                      )}
                      {share.expires_at && (
                        <span className="flex items-center gap-1">
                          <Clock className="w-3 h-3" /> Expires: {new Date(share.expires_at).toLocaleString()}
                        </span>
                      )}
                      <span className="ml-auto">
                        Created: {new Date(share.created_at).toLocaleDateString()}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

      </div>
    </div>
  );
}
