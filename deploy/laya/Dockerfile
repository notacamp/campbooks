# laya-serve: the `decide` role's default model server (ADR 0009). Optional,
# used by the compose `ai` profile; Campbooks itself does not need it.
# Published by the release pipeline as ghcr.io/notacamp/campbooks-laya, with
# the same tags as the server image (.github/workflows/image.yml).
#
#   docker build -t campbooks-laya deploy/laya
#
# CPU-only (linux/amd64, linux/arm64). The checkpoint is not baked in: the
# server downloads it into $HF_HOME (a volume) on first start.

ARG PYTHON_IMAGE=python:3.12.14-slim-bookworm
ARG TORCH_VERSION=2.14.0

FROM ${PYTHON_IMAGE}
ARG TORCH_VERSION

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    # transformers can hang importing TensorFlow when it is present (Laya README).
    USE_TF=0 \
    HF_HOME=/models \
    HF_HUB_DISABLE_TELEMETRY=1 \
    LAYA_HOST=0.0.0.0 \
    LAYA_PORT=8000 \
    LAYA_DEVICE=cpu \
    # Load the multilingual checkpoint at startup (downloaded once into the
    # volume), so the first request does not pay for it.
    LAYA_PRELOAD=1 \
    LAYA_MODELS=multilingual \
    # Download the checkpoint commit the laya package lists as reviewed.
    LAYA_REVISION=reviewed

# Debian security fixes released after the base image was built (the release
# pipeline's Trivy gate fails on fixable HIGH/CRITICAL findings). APT_REFRESH
# is the build day, passed by CI and the release pipeline: a new value reruns
# this step, so a cached layer never keeps a library Debian has since fixed
# (laya:next shipped libssl3 3.0.22-1~deb12u1 from a cached layer while
# bookworm-security had deb12u2, CVE-2026-84782).
ARG APT_REFRESH=unset
RUN echo "Debian packages as of ${APT_REFRESH}" \
    && apt-get update \
    && apt-get upgrade -y --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 10001 laya \
    && useradd --uid 10001 --gid laya --no-create-home --shell /usr/sbin/nologin laya \
    && mkdir -p /models \
    && chown laya:laya /models

COPY requirements.txt /tmp/requirements.txt
RUN pip install --index-url https://download.pytorch.org/whl/cpu "torch==${TORCH_VERSION}" \
    && pip install -r /tmp/requirements.txt \
    && rm /tmp/requirements.txt

COPY --chmod=0755 entrypoint.sh /usr/local/bin/laya-entrypoint

USER laya
WORKDIR /models
EXPOSE 8000
HEALTHCHECK --interval=10s --timeout=5s --start-period=600s --retries=6 \
    CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/health', timeout=4)"]
ENTRYPOINT ["laya-entrypoint"]
