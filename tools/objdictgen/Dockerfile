FROM ubuntu:16.04

LABEL "RHS"="Robotics Hardware Solutions"
LABEL description="Image for CAN Open dictionary editor"
LABEL maintainer="doc.pillman@icloud.com"

RUN apt-get update

RUN apt-get install -y --no-install-recommends\
    cmake\
    python-wxgtk3.0\
    python2.7-minimal\
    python-pip\
    wget\
    unzip\
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install build essentials and development tools
RUN apt-get update && apt-get install -y \
    build-essential \
    gcc \
    g++ \
    make \
    zlib1g-dev \
    libjpeg-dev \
    libpng-dev \
    libfreetype6-dev \
    liblcms2-dev \
    libwebp-dev \
    tcl8.6-dev \
    tk8.6-dev \
    python-dev \
    python-tk \
    xvfb \
    x11vnc \
    fluxbox \
    && rm -rf /var/lib/apt/lists/*

# Install system dependencies
RUN apt-get update && apt-get install -y \
    zlib1g-dev libjpeg-dev libpng-dev libfreetype6-dev \
    && rm -rf /var/lib/apt/lists/*

# Install setuptools first, then upgrade pip
RUN pip install --no-cache-dir setuptools==44.1.1
RUN pip install --no-cache-dir --upgrade "pip<21.0"

# Install wxpython for graphical interface and prerequsities
RUN pip --disable-pip-version-check install attrdict pillow==3.4 wxpython

# Download the object disctionary editor & generator
RUN wget https://github.com/RoboticsHardwareSolutions/co_objdictgen/archive/refs/heads/main.zip -O co_objdictgen-main.zip \
    && unzip co_objdictgen-main.zip \
    && rm co_objdictgen-main.zip

WORKDIR /co_objdictgen-main

RUN tar -xzf Gnosis*.tar.gz
RUN cd Gnosis*\
    && python2.7 setup.py install\
    && cd -

WORKDIR /workdir

# VNC setup
EXPOSE 5900
RUN mkdir ~/.vnc && x11vnc -storepasswd 1234 ~/.vnc/passwd

# Start script
RUN echo '#!/bin/bash\n\
Xvfb :1 -screen 0 1024x768x16 &\n\
fluxbox -display :1 &\n\
x11vnc -forever -usepw -create -display :1 &\n\
export DISPLAY=:1\n\
exec "$@"' > /start.sh && chmod +x /start.sh

ENTRYPOINT [ "/start.sh", "python2.7", "/co_objdictgen-main/objdictedit.py" ]
