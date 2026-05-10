/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "standalone",
  typedRoutes: true,
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
