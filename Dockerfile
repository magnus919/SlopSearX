# SlopSearX production image
# Target: ~200MB, cold start <2s, Python 3.12
FROM python:3.12-slim

LABEL org.opencontainers.image.title="SlopSearX"
LABEL org.opencontainers.image.description="Cloud-native, stateless, AI-agent-first meta search engine"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# System deps for lxml
RUN apt-get update && apt-get install -y --no-install-recommends libxml2 libxslt1.1 && rm -rf /var/lib/apt/lists/*

# Python deps
COPY pyproject.toml .
RUN pip install --no-cache-dir -e ".[dev]" && pip uninstall -y pytest ruff cssselect

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
