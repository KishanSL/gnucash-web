# Stage 1: Build frontend from source in pristine environment
FROM docker.io/library/node:22-alpine AS builder
WORKDIR /app

ENV PUPPETEER_SKIP_DOWNLOAD=true

# Cache dependency layer
COPY package.json package-lock.json ./
RUN npm ci

# Copy full frontend source code and compile production assets
COPY . .
RUN npm run build

# Stage 2: Minimal runtime web server
FROM docker.io/library/nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
