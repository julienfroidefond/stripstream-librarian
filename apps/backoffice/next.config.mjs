/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "standalone",
  typedRoutes: true,
  images: {
    minimumCacheTTL: 86400,
  },
};

export default nextConfig;
