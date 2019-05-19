FROM lambci/lambda:build-provided

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

WORKDIR /app

ENV PATH "/root/.cargo/bin:${PATH}"
ENV CARGO_HOME /app/.cargo/cache

ENTRYPOINT TARGET_NATIVE=1 make debug
