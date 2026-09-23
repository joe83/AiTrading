# ==============================================================================
# Multi-Stage Dockerfile for AI Trading Web Dashboard (React + Vite)
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Build static assets
# ------------------------------------------------------------------------------
FROM node:22-alpine AS builder

WORKDIR /app

# Install dependencies first (leverage layer cache)
COPY package.json package-lock.json* ./
RUN npm ci || npm install

# Copy source code and build. Drop the local .env so the production bundle
# calls the same host that served the page, not localhost:8080.
COPY . .
RUN rm -f .env .env.local .env.production && npm run build

# ------------------------------------------------------------------------------
# Stage 2: Serve with Nginx Alpine
# ------------------------------------------------------------------------------
FROM nginx:alpine-slim AS runner

# Remove default html & config
RUN rm -rf /usr/share/nginx/html/* /etc/nginx/conf.d/default.conf

# Copy custom Nginx configuration
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Copy compiled SPA from builder
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80

HEALTHCHECK --interval=20s --timeout=5s --retries=3 \
  CMD wget -q --spider http://127.0.0.1:80/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
