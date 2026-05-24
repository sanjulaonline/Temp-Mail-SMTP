import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";

const inter = Inter({
  subsets: ["latin"],
  weight: ["300", "400", "500", "600", "700"],
});

const SITE_URL = "https://phantom-mail.sanjula.online";
const AUTHOR_URL = "https://www.sanjula.online";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: "Phantom Mail — Free Disposable Temporary Email",
    template: "%s | Phantom Mail",
  },
  description:
    "Create free disposable temporary email addresses instantly. No signup required. Protect your privacy with Phantom Mail — a fast, open-source temp mail service built in Rust.",
  keywords: [
    "disposable email",
    "temporary email",
    "temp mail",
    "fake email",
    "throwaway email",
    "anonymous email",
    "free temp email",
    "10 minute mail",
    "guerrilla mail alternative",
    "phantom mail",
    "privacy email",
    "spam protection email",
  ],
  authors: [{ name: "Sanjula", url: AUTHOR_URL }],
  creator: "Sanjula",
  publisher: "Sanjula",
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      "max-image-preview": "large",
      "max-snippet": -1,
    },
  },
  alternates: {
    canonical: SITE_URL,
  },
  openGraph: {
    type: "website",
    url: SITE_URL,
    siteName: "Phantom Mail",
    title: "Phantom Mail — Free Disposable Temporary Email",
    description:
      "Create free disposable temporary email addresses instantly. No signup. No tracking. Protect your privacy.",
    locale: "en_US",
    images: [
      {
        url: `${SITE_URL}/og-image.png`,
        width: 1200,
        height: 630,
        alt: "Phantom Mail — Free Disposable Email Service",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Phantom Mail — Free Disposable Temporary Email",
    description:
      "Create free disposable temporary email addresses instantly. No signup. No tracking.",
    images: [`${SITE_URL}/og-image.png`],
    creator: "@sanjula",
  },
  category: "technology",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <head>
        {/* GEO meta tags */}
        <meta name="geo.region" content="US" />
        <meta name="geo.placename" content="United States" />
        <meta name="language" content="English" />

        {/* Structured data — WebApplication */}
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{
            __html: JSON.stringify({
              "@context": "https://schema.org",
              "@type": "WebApplication",
              name: "Phantom Mail",
              url: SITE_URL,
              description:
                "Free disposable temporary email service. Create anonymous throwaway email addresses instantly with no signup required.",
              applicationCategory: "UtilitiesApplication",
              operatingSystem: "Any",
              offers: {
                "@type": "Offer",
                price: "0",
                priceCurrency: "USD",
              },
              author: {
                "@type": "Person",
                name: "Sanjula",
                url: AUTHOR_URL,
              },
              creator: {
                "@type": "Person",
                name: "Sanjula",
                url: AUTHOR_URL,
              },
            }),
          }}
        />
      </head>
      <body className={inter.className}>
        {children}
      </body>
    </html>
  );
}
