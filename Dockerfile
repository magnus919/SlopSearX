# SlopSearX production image
# Target: ~200MB, cold start <2s, Python 3.14
# Dependabot maintains this versioned Docker Official Image tag and digest.
FROM python:3.14.7-slim-trixie@sha256:cad9a2c871761c413caa6fdd6441c783451e740a48aaeba60ae62a8b53525ef6

LABEL org.opencontainers.image.title="SlopSearX"
LABEL org.opencontainers.image.description="Cloud-native, stateless, AI-agent-first meta search engine"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# System deps for lxml
RUN apt-get update \
    && apt-get install -y --no-install-recommends libxml2 libxslt1.1 \
    && rm -rf /var/lib/apt/lists/*

# Python deps
COPY pyproject.toml .
RUN pip install --no-cache-dir -e .

# Apply base-image security updates after dependency installation so the
# per-build refresh does not invalidate the expensive pip layer.
ARG DEBIAN_SECURITY_REFRESH
RUN : "${DEBIAN_SECURITY_REFRESH:?set a unique DEBIAN_SECURITY_REFRESH build argument}" \
    && echo "Debian security refresh: ${DEBIAN_SECURITY_REFRESH}" \
    && apt-get update \
    && apt-get dist-upgrade -y --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# pip is needed only while building. Remove it from the runtime image so its
# vendored libraries cannot reintroduce vulnerable build-time dependencies.
RUN rm -rf \
    /usr/local/lib/python3.14/site-packages/pip \
    /usr/local/lib/python3.14/site-packages/pip-*.dist-info \
    /usr/local/bin/pip \
    /usr/local/bin/pip3 \
    /usr/local/bin/pip3.14

# Application code
COPY slopsearx/ slopsearx/
COPY engines/ engines/

# Non-root user
RUN useradd --create-home --shell /bin/bash slopsearx
USER slopsearx

EXPOSE 8080

# Health check script
COPY docker/healthcheck.py /app/healthcheck.py
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 CMD python /app/healthcheck.py

CMD ["uvicorn", "slopsearx.server:app", "--host", "0.0.0.0", "--port", "8080"]
