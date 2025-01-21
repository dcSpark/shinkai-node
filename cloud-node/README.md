# Shinkai Node Cloud Deployment Guide

This guide explains how to deploy a Shinkai Node on various cloud providers. The setup includes running the node as a systemd service with Docker and configuring HTTPS access.

## Prerequisites

- Docker installed on the VM
- A domain name for HTTPS setup
- Basic knowledge of cloud platform management

## Files Structure
```
cloud-node/
├── Dockerfile         # Docker image definition
├── env.conf          # Environment variables configuration
└── shinkai-node.service  # Systemd service definition
```

## General Setup Steps

1. Create the necessary directories:
```bash
sudo mkdir -p /opt/shinkai-node
```

2. Copy configuration files:
```bash
sudo cp env.conf /opt/shinkai-node/
sudo cp shinkai-node.service /etc/systemd/system/
```

3. Configure environment variables:
```bash
sudo nano /opt/shinkai-node/env.conf
# Set your IDENTITY_SECRET_KEY, ENCRYPTION_SECRET_KEY, EMBEDDINGS_SERVER_URL, and INITIAL_AGENT_API_KEYS
```

4. Enable and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable shinkai-node
sudo systemctl start shinkai-node
```

## Cloud-Specific Setup

### Google Cloud Platform (GCP)

1. Create VM Instance:
```bash
gcloud compute instances create shinkai-node \
  --machine-type=e2-standard-2 \
  --image-family=debian-11 \
  --image-project=debian-cloud \
  --boot-disk-size=50GB \
  --tags=http-server,https-server
```

2. Configure Firewall:
```bash
# Allow HTTP and HTTPS traffic
gcloud compute firewall-rules create allow-http \
  --allow tcp:80 \
  --target-tags=http-server

gcloud compute firewall-rules create allow-https \
  --allow tcp:443 \
  --target-tags=https-server
```

3. Install Nginx and Certbot:
```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
```

4. Configure Nginx (replace `your-domain.com`):
```nginx
# HTTP redirect to HTTPS
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}

# HTTPS server
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;

    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384;
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:50m;
    ssl_stapling on;
    ssl_stapling_verify on;
    add_header Strict-Transport-Security max-age=15768000;

    location / {
        proxy_pass http://localhost:9550;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Amazon Web Services (AWS)

1. Launch EC2 Instance:
   - AMI: Debian 11
   - Instance Type: t3.medium
   -- Storage: 50GB gp3

2. Configure Security Group:
   - Allow TCP 80 (HTTP)
   - Allow TCP 443 (HTTPS)
   - Ensure 9550 is NOT exposed to the internet

3. Configure Route 53 (if using AWS domains) or point your domain's A record to the EC2 instance.

4. Install and configure SSL:
```bash
sudo apt update
sudo apt install -y nginx certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com
```

### Microsoft Azure

1. Create VM:
```bash
az vm create \
  --resource-group shinkai-group \
  --name shinkai-node \
  --image Debian:debian-11:11:latest \
  --size Standard_B2s \
  --admin-username azureuser \
  --generate-ssh-keys
```

2. Configure Network Security Group:
```bash
# Allow HTTP
az network nsg rule create \
  --resource-group shinkai-group \
  --nsg-name shinkai-node-nsg \
  --name allow-http \
  --protocol tcp \
  --priority 1001 \
  --destination-port-range 80

# Allow HTTPS
az network nsg rule create \
  --resource-group shinkai-group \
  --nsg-name shinkai-node-nsg \
  --name allow-https \
  --protocol tcp \
  --priority 1002 \
  --destination-port-range 443
```

3. Follow the same Nginx and SSL setup as GCP.

### DigitalOcean

1. Create Droplet:
   - Choose Debian 11
   - Basic Plan (Regular Intel with SSD)
   - 2GB/2CPU minimum
   - Choose datacenter region
   - Add SSH key

2. Configure Firewall:
   - Create new firewall
   - Allow TCP 80 (HTTP)
   - Allow TCP 443 (HTTPS)
   - Do NOT expose port 9550 to the internet
   - Apply to Shinkai Node droplet

3. Follow the same Nginx and SSL setup as above.

## SSL/TLS Configuration

For all platforms, configure SSL with Let's Encrypt:

1. Install Certbot:
```bash
sudo apt install -y certbot python3-certbot-nginx
```

2. Obtain certificate:
```bash
sudo certbot --nginx -d your-domain.com
```

3. Configure auto-renewal:
```bash
sudo systemctl enable certbot.timer
sudo systemctl start certbot.timer
```

## Monitoring and Maintenance

Check service status:
```bash
sudo systemctl status shinkai-node
```

View logs:
```bash
sudo journalctl -u shinkai-node -f
```

Update the container:
```bash
sudo docker pull dcspark/shinkai-node:latest
sudo systemctl restart shinkai-node
```

## Security Considerations

1. Keep your environment variables secure
2. Regularly update your system and Docker images
3. Monitor system resources and logs
4. Use strong SSL/TLS configuration
5. Consider implementing rate limiting
6. Backup your storage directory regularly
7. Ensure port 9550 is only accessible locally
8. Keep Nginx and SSL certificates up to date
9. Enable and configure firewall (ufw) on the host:
```bash
sudo apt install ufw
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow ssh
sudo ufw enable
```

## Troubleshooting

1. Check service status:
```bash
sudo systemctl status shinkai-node
```

2. View detailed logs:
```bash
sudo journalctl -u shinkai-node -f
```

3. Verify container is running:
```bash
docker ps | grep shinkai-node
```

4. Test API endpoint:
```bash
curl -k https://your-domain.com/v2/health_check
```

5. Check Nginx configuration:
```bash
sudo nginx -t
```

## Support

For additional support or questions, please refer to the Shinkai documentation or contact support. 
