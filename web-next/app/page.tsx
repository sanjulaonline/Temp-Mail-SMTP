"use client";

import { useCallback, useEffect, useRef, useState } from "react";

type ApiResponse<T> = {
  success: boolean;
  data: T | null;
  error: string | null;
};

type TemporaryMailbox = {
  email_address: string;
  created_at: string;
  expires_at: string;
};

type Email = {
  id: string;
  sender: string;
  recipient: string;
  subject: string;
  body: string;
  timestamp: string;
};

function formatBody(raw: string): { text: string; quoted: boolean }[] {
  return raw
    .replace(/\r\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd()
    .split("\n")
    .map((line) => ({ text: line, quoted: line.startsWith(">") }));
}

function fmtDate(ts: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const diff = now.getTime() - d.getTime();

  if (diff < 60000) return "just now";
  if (diff < 3600000) return Math.floor(diff / 60000) + "m ago";
  if (diff < 86400000) return Math.floor(diff / 3600000) + "h ago";
  return d.toLocaleDateString();
}

export default function Home() {
  const [view, setView] = useState<"generate" | "inbox" | "compose" | "reader">(
    "generate",
  );

  const [currentEmail, setCurrentEmail] = useState<string | null>(null);
  const [expiresAt, setExpiresAt] = useState<Date | null>(null);
  const [emails, setEmails] = useState<Email[]>([]);
  const [selectedEmail, setSelectedEmail] = useState<Email | null>(null);

  const [isGenerating, setIsGenerating] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [composeTo, setComposeTo] = useState("");
  const [composeSubject, setComposeSubject] = useState("");
  const [composeBody, setComposeBody] = useState("");
  const [refreshLabel, setRefreshLabel] = useState("Checking…");
  const [copyLabel, setCopyLabel] = useState("Copy");

  const [toastMessage, setToastMessage] = useState<string>("");
  const [toastVisible, setToastVisible] = useState(false);
  const toastTimerRef = useRef<number | null>(null);

  const copyTimerRef = useRef<number | null>(null);
  const ttlTimerRef = useRef<number | null>(null);
  const pollTimerRef = useRef<number | null>(null);

  const [ttlPercent, setTtlPercent] = useState(100);
  const [ttlTime, setTtlTime] = useState("24h");

  const showToast = useCallback((msg: string) => {
    setToastMessage(msg);
    setToastVisible(true);

    if (toastTimerRef.current) window.clearTimeout(toastTimerRef.current);
    toastTimerRef.current = window.setTimeout(() => {
      setToastVisible(false);
      toastTimerRef.current = null;
    }, 3000);
  }, []);

  const clearTimers = useCallback(() => {
    if (pollTimerRef.current !== null) {
      window.clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
    if (ttlTimerRef.current !== null) {
      window.clearInterval(ttlTimerRef.current);
      ttlTimerRef.current = null;
    }
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = null;
    }
  }, []);

  async function generateMailbox() {
    setIsGenerating(true);
    try {
      const resp = await fetch("/mailboxes", { method: "POST" });
      const json = (await resp.json()) as ApiResponse<TemporaryMailbox>;

      if (!json.success || !json.data) {
        throw new Error(json.error || "Failed to create mailbox");
      }

      setCurrentEmail(json.data.email_address);
      setExpiresAt(new Date(json.data.expires_at));

      setEmails([]);
      setSelectedEmail(null);
      setRefreshLabel("Checking…");
      setCopyLabel("Copy");
      setTtlPercent(100);
      setTtlTime("24h");

      setView("inbox");
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Something went wrong";
      showToast("⚠ " + msg);
    } finally {
      setIsGenerating(false);
    }
  }

  async function copyEmail() {
    if (!currentEmail) return;
    try {
      await navigator.clipboard.writeText(currentEmail);
      setCopyLabel("Copied!");
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
      copyTimerRef.current = window.setTimeout(() => {
        setCopyLabel("Copy");
        copyTimerRef.current = null;
      }, 2000);
    } catch {
      showToast("Could not copy");
    }
  }

  function reset() {
    clearTimers();
    setCurrentEmail(null);
    setExpiresAt(null);
    setEmails([]);
    setSelectedEmail(null);
    setRefreshLabel("Checking…");
    setCopyLabel("Copy");
    setTtlPercent(100);
    setTtlTime("24h");
    setIsGenerating(false);
    setView("generate");
  }

  function showInbox() {
    setView("inbox");
  }

  async function sendEmail() {
    if (!currentEmail) return;
    setIsSending(true);
    try {
      const encoded = encodeURIComponent(currentEmail);
      const resp = await fetch(`/mailboxes/${encoded}/send`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ to: composeTo, subject: composeSubject, body: composeBody }),
      });
      const json = (await resp.json()) as ApiResponse<null>;
      if (!json.success) throw new Error(json.error || "Failed to send");
      showToast("Email sent!");
      setComposeTo("");
      setComposeSubject("");
      setComposeBody("");
      setView("inbox");
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Failed to send";
      showToast("⚠ " + msg);
    } finally {
      setIsSending(false);
    }
  }

  function openEmail(idx: number) {
    const e = emails[idx];
    if (!e) return;
    setSelectedEmail(e);
    setView("reader");
  }

  // ── TTL bar ───────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!expiresAt) return;
    const total = expiresAt.getTime() - new Date().getTime();

    const updateTtl = () => {
      const remaining = Math.max(0, expiresAt.getTime() - new Date().getTime());
      const pct = total > 0 ? (remaining / total) * 100 : 0;
      setTtlPercent(Math.max(0, Math.min(100, pct)));

      const h = Math.floor(remaining / 3600000);
      const m = Math.floor((remaining % 3600000) / 60000);
      const s = Math.floor((remaining % 60000) / 1000);
      setTtlTime(h > 0 ? `${h}h ${m}m` : m > 0 ? `${m}m ${s}s` : `${s}s`);

      if (remaining === 0) {
        if (ttlTimerRef.current !== null) {
          window.clearInterval(ttlTimerRef.current);
          ttlTimerRef.current = null;
        }
        showToast("Mailbox expired");
      }
    };

    updateTtl();
    ttlTimerRef.current = window.setInterval(updateTtl, 1000);
    return () => {
      if (ttlTimerRef.current !== null) {
        window.clearInterval(ttlTimerRef.current);
        ttlTimerRef.current = null;
      }
    };
  }, [expiresAt, showToast]);

  // ── Polling ───────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!currentEmail) return;
    const emailAddress = currentEmail;
    let cancelled = false;
    const controller = new AbortController();

    const fetchEmails = async () => {
      if (cancelled) return;
      setRefreshLabel("Checking…");
      try {
        const encoded = encodeURIComponent(emailAddress);
        const resp = await fetch(`/mailboxes/${encoded}/emails`,
          { signal: controller.signal },
        );

        const json = (await resp.json()) as ApiResponse<Email[]>;
        if (cancelled) return;
        setRefreshLabel("Live · updated just now");
        if (json.success) setEmails(json.data || []);
      } catch {
        if (cancelled) return;
        setRefreshLabel("Offline");
      }
    };

    fetchEmails();
    pollTimerRef.current = window.setInterval(fetchEmails, 5000);
    return () => {
      cancelled = true;
      controller.abort();
      if (pollTimerRef.current !== null) {
        window.clearInterval(pollTimerRef.current);
        pollTimerRef.current = null;
      }
    };
  }, [currentEmail]);

  return (
    <>
      <div className="app">
        <header>
          <div className="logo">
            <span className="logo-icon">👻</span>
            <span className="logo-text">Phantom Mail</span>
          </div>
          <p>
            Temporary Email Service written in Rust. Create disposable email
            addresses instantly.
          </p>
        </header>

        <section
          id="generate-section"
          className="card"
          style={{ display: view === "generate" ? "block" : "none" }}
        >
          <div style={{ marginBottom: 20 }}>
            <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 6 }}>
              Get a Disposable Inbox
            </h2>
            <p style={{ fontSize: 14, color: "var(--muted)" }}>
              Instantly generate a temporary email. No signup. No tracking.
            </p>
          </div>
          <button
            id="gen-btn"
            className="generate-btn"
            onClick={generateMailbox}
            disabled={isGenerating}
          >
            {isGenerating ? <span className="spinner" /> : "✦ Generate Email Address"}
          </button>
        </section>

        <section
          id="inbox-section"
          style={{ display: view === "inbox" ? "block" : "none" }}
        >
          <div className="card">
            <div className="inbox-header">
              <h2>Your Inbox</h2>
              <div className="refresh-row">
                <div className="refresh-dot" />
                <span className="refresh-label" id="refresh-label">
                  {refreshLabel}
                </span>
                <button className="compose-btn" onClick={() => setView("compose")}>
                  ✉ Compose
                </button>
                <button className="new-btn" onClick={reset}>
                  + New Address
                </button>
              </div>
            </div>

            <div className="email-box">
              <span className="email-addr" id="email-display">
                {currentEmail ?? ""}
              </span>
              <button className="copy-btn" onClick={copyEmail} id="copy-btn">
                {copyLabel}
              </button>
            </div>

            <div className="ttl-bar">
              <span className="ttl-label">Expires in</span>
              <div className="ttl-track">
                <div
                  className="ttl-fill"
                  id="ttl-fill"
                  style={{ width: `${ttlPercent}%` }}
                />
              </div>
              <span className="ttl-time" id="ttl-time">
                {ttlTime}
              </span>
            </div>

            <div className="mail-list" id="mail-list">
              {emails.length === 0 ? (
                <div className="empty-state">
                  <div className="empty-icon">📭</div>
                  <p>No emails yet. Share your address and wait for messages.</p>
                </div>
              ) : (
                emails.map((e, i) => (
                  <div
                    key={e.id}
                    className="mail-item"
                    onClick={() => openEmail(i)}
                    data-idx={i}
                  >
                    <div className="mail-item-top">
                      <span className="mail-from">{e.sender}</span>
                      <span className="mail-time">{fmtDate(e.timestamp)}</span>
                    </div>
                    <div className="mail-subject">{e.subject}</div>
                  </div>
                ))
              )}
            </div>
          </div>
        </section>

        <section
          id="compose-section"
          style={{ display: view === "compose" ? "block" : "none" }}
        >
          <button className="back-btn" onClick={showInbox}>
            ← Back to Inbox
          </button>
          <div className="card">
            <div style={{ marginBottom: 20 }}>
              <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 6 }}>New Message</h2>
              <p className="compose-from">
                From: <span>{currentEmail ?? ""}</span>
              </p>
            </div>
            <div className="compose-form">
              <div className="form-field">
                <label>To</label>
                <input
                  type="email"
                  placeholder="recipient@example.com"
                  value={composeTo}
                  onChange={(e) => setComposeTo(e.target.value)}
                />
              </div>
              <div className="form-field">
                <label>Subject</label>
                <input
                  type="text"
                  placeholder="Enter subject…"
                  maxLength={200}
                  value={composeSubject}
                  onChange={(e) => setComposeSubject(e.target.value)}
                />
              </div>
              <div className="form-field">
                <label>Message</label>
                <textarea
                  placeholder="Write your message…"
                  value={composeBody}
                  onChange={(e) => setComposeBody(e.target.value)}
                />
              </div>
              <div className="compose-actions">
                <button className="cancel-btn" onClick={showInbox}>Cancel</button>
                <button
                  className="send-btn"
                  onClick={sendEmail}
                  disabled={isSending || !composeTo || !composeSubject || !composeBody}
                >
                  {isSending ? <span className="spinner" /> : "Send"}
                </button>
              </div>
            </div>
          </div>
        </section>

        <section
          id="reader-section"
          style={{ display: view === "reader" ? "block" : "none" }}
        >
          <button className="back-btn" onClick={showInbox}>
            ← Back to Inbox
          </button>
          <div className="card">
            <div className="reader-meta">
              <div className="reader-subject" id="reader-subject">
                {selectedEmail?.subject ?? ""}
              </div>
              <div className="reader-from">
                From: <span id="reader-from">{selectedEmail?.sender ?? ""}</span>
                &nbsp;·&nbsp;
                <span id="reader-time" style={{ color: "var(--muted)" }}>
                  {selectedEmail ? fmtDate(selectedEmail.timestamp) : ""}
                </span>
              </div>
            </div>
            <div className="reader-body" id="reader-body">
              {selectedEmail
                ? formatBody(selectedEmail.body).map((line, i) => (
                    <span
                      key={i}
                      style={line.quoted ? { color: "var(--muted)", display: "block" } : { display: "block" }}
                    >
                      {line.text || " "}
                    </span>
                  ))
                : "(no body)"}
            </div>
          </div>
        </section>
      </div>

      <div id="toast" className={toastVisible ? "show" : ""}>
        {toastMessage}
      </div>
    </>
  );
}
