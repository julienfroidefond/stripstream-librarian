/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "standalone",
  typedRoutes: true,
  images: {
    minimumCacheTTL: 86400,
    unoptimized: true,
  },
};

export default nextConfig;
