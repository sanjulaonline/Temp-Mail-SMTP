import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  async rewrites() {
    const backendBaseUrl =
      process.env.BACKEND_HTTP_BASE_URL ?? "http://localhost:8080";

    return [
      { source: "/mailboxes", destination: `${backendBaseUrl}/mailboxes` },
      {
        source: "/mailboxes/:path*",
        destination: `${backendBaseUrl}/mailboxes/:path*`,
      },
    ];
  },
};

export default nextConfig;
