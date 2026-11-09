# Stage 1: Build
FROM node:26-alpine AS build
WORKDIR /app
COPY apps/docs/package.json apps/docs/package-lock.json ./
RUN npm ci
COPY apps/docs/ ./
RUN npm run build

# Stage 2: Serve static files with nginx
FROM nginx:alpine

# Patch packages shipped by the mutable nginx:alpine base image: its apk
# snapshot can lag behind the repository (e.g. zlib CVE-2026-4738, fixed in
# 1.4-r0). APK_CACHE_BUST changes on every CI run so this layer is not served
# from the registry cache (a plain RUN would keep the old package forever).
ARG APK_CACHE_BUST=local
RUN apk upgrade --no-cache zlib

COPY --from=build /app/dist /usr/share/nginx/html
COPY apps/docs/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 7084
