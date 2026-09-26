/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "standalone",
  typedRoutes: true,
  // Pin the workspace root to this app. The repo root and apps/docs also ship
  // lockfiles, so Turbopack otherwise infers the monorepo root and nests the
  // standalone build under .next/standalone/apps/backoffice/.
  turbopack: {
    root: import.meta.dirname,
  },
  images: {
    minimumCacheTTL: 86400,
    unoptimized: true,
  },
  experimental: {
    staleTimes: {
      dynamic: 10,
      static: 60,
    },
  },
};

export default nextConfig;
