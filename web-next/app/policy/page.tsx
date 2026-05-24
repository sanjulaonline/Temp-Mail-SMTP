import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Email & Privacy Policy",
  description:
    "Phantom Mail email policy, privacy information, and terms of use. Learn how we handle temporary email addresses and outbound messages.",
  robots: { index: true, follow: true },
  alternates: { canonical: "https://phantom-mail.sanjula.online/policy" },
};

export default function PolicyPage() {
  return (
    <div className="policy-page">
      <div className="policy-container">
        <Link href="/" className="policy-back">← Back to Phantom Mail</Link>

        <h1>Email &amp; Privacy Policy</h1>
        <p className="policy-updated">Last updated: May 2026</p>

        <section>
          <h2>1. What is Phantom Mail?</h2>
          <p>
            Phantom Mail is a free, open-source disposable email service. It lets
            anyone generate a temporary email address instantly — no account, no
            signup, no tracking. Addresses expire automatically after 24 hours.
          </p>
        </section>

        <section>
          <h2>2. Outbound Email — User Responsibility</h2>
          <p>
            Phantom Mail provides the ability to send emails from a generated
            temporary address. <strong>All outbound emails are composed and sent
            entirely by the user.</strong> The operator of this service (Sanjula)
            has no knowledge of, and bears no responsibility for, the content of
            any emails sent through this platform.
          </p>
          <p>
            By using the send feature you agree that:
          </p>
          <ul>
            <li>You will not send spam, unsolicited bulk email, or phishing messages.</li>
            <li>You will not use this service to harass, threaten, or defraud any person.</li>
            <li>You will not send content that violates applicable laws.</li>
            <li>You accept full responsibility for the content you send.</li>
          </ul>
          <p>
            The service enforces a hard limit of <strong>5 outbound emails per IP
            address per day</strong> and <strong>5 outbound emails per mailbox per
            day</strong> to prevent abuse.
          </p>
        </section>

        <section>
          <h2>3. Data We Store</h2>
          <p>
            We store only what is necessary to operate the service:
          </p>
          <ul>
            <li>The generated email address and its expiry time.</li>
            <li>Inbound emails delivered to that address (subject, sender, body).</li>
          </ul>
          <p>
            All data is <strong>automatically deleted when the mailbox expires</strong>{" "}
            (within 24 hours of creation). We do not store your IP address, browser
            fingerprint, or any personally identifiable information beyond what
            appears in received emails.
          </p>
        </section>

        <section>
          <h2>4. No Tracking</h2>
          <p>
            Phantom Mail contains no advertising, no analytics scripts, no cookies,
            and no third-party trackers. The only external service used is{" "}
            <a href="https://resend.com" target="_blank" rel="noopener noreferrer">
              Resend
            </a>{" "}
            as an SMTP relay for outbound email delivery.
          </p>
        </section>

        <section>
          <h2>5. Abuse Reporting</h2>
          <p>
            If you received an unwanted or abusive email originating from a{" "}
            <code>@mail.phantom-mail.sanjula.online</code> address, please report
            it to{" "}
            <a href="mailto:sanjula692@gmail.com">sanjula692@gmail.com</a>{" "}
            with the full email headers. Reported addresses will be investigated
            and blocked.
          </p>
        </section>

        <section>
          <h2>6. Disclaimer</h2>
          <p>
            This service is provided <strong>as-is</strong> without warranty of
            any kind. Temporary email addresses are public — anyone who knows your
            address can send to it. Do not use Phantom Mail for sensitive or
            confidential communications.
          </p>
        </section>

        <section>
          <h2>7. Contact</h2>
          <p>
            For any questions, abuse reports, or concerns contact the developer:
          </p>
          <p>
            <strong>Sanjula</strong><br />
            <a href="mailto:sanjula692@gmail.com">sanjula692@gmail.com</a><br />
            <a href="https://www.sanjula.online" target="_blank" rel="noopener noreferrer">
              www.sanjula.online
            </a>
          </p>
        </section>
      </div>
    </div>
  );
}
