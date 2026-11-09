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

# The standalone server only needs `node`: drop npm/npx (and the vulnerable
# packages they bundle, e.g. undici / brace-expansion reported by image scans).
RUN rm -rf /usr/local/lib/node_modules/npm \
    /usr/local/bin/npm /usr/local/bin/npx

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public
EXPOSE 7082
CMD ["node", "server.js"]
