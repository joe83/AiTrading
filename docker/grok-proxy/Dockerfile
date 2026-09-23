FROM python:3.12-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir git+https://github.com/trinity-cloud/grok-proxy.git

WORKDIR /opt/grok-proxy
COPY account_control.py /opt/grok-proxy/account_control.py
COPY start.sh /opt/grok-proxy/start.sh
RUN chmod +x /opt/grok-proxy/start.sh

ENV GROK_PROXY_HOME=/data
EXPOSE 8585 8586

CMD ["bash", "/opt/grok-proxy/start.sh"]
