FROM node:26-alpine AS deps
WORKDIR /app
COPY apps/backoffice/package.json apps/backoffice/package-lock.json ./
RUN npm ci

FROM node:26-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY apps/backoffice/ ./
RUN npm run build

FROM node:26-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=7082
ENV HOST=0.0.0.0

# Patch packages shipped by the mutable node:26-alpine base image: its apk
# snapshot can lag behind the repository (e.g. zlib CVE-2026-4738, fixed in
# 1.4-r0). APK_CACHE_BUST changes on every CI run so this layer is not served
# from the registry cache (a plain RUN would keep the old package forever).
ARG APK_CACHE_BUST=local
RUN apk upgrade --no-cache zlib

# The standalone server only needs `node`: drop npm/npx (and the vulnerable
# packages they bundle, e.g. undici / brace-expansion / minimatch reported by
# image scans).
RUN rm -rf /usr/local/lib/node_modules/npm \
    /usr/local/bin/npm /usr/local/bin/npx

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public
EXPOSE 7082
CMD ["node", "server.js"]
