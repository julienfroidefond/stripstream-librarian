# Stage 1: Build
FROM node:22-alpine AS build
WORKDIR /app
COPY apps/docs/package.json apps/docs/package-lock.json ./
RUN npm ci
COPY apps/docs/ ./
RUN npm run build

# Stage 2: Serve static files with nginx
FROM nginx:alpine
COPY --from=build /app/dist /usr/share/nginx/html
COPY apps/docs/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 7084
