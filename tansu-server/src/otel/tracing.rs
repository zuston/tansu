// Copyright ⓒ 2024-2025 Peter Morgan <peter.james.morgan@gmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::{Result, TracingFormat};
use opentelemetry::KeyValue;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{
    Resource,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use opentelemetry_semantic_conventions::{
    SCHEMA_URL,
    resource::{SERVICE_NAME, SERVICE_VERSION},
};
use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

fn resource() -> Resource {
    Resource::builder()
        .with_schema_url(
            [
                KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
                KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            ],
            SCHEMA_URL,
        )
        .build()
}

fn init_tracer_provider() -> Result<SdkTracerProvider> {
    SpanExporter::builder()
        .with_tonic()
        .build()
        .map_err(Into::into)
        .map(|exporter| {
            SdkTracerProvider::builder()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    1.0,
                ))))
                .with_resource(resource())
                .with_batch_exporter(exporter)
                .with_id_generator(RandomIdGenerator::default())
                .build()
        })
}

#[derive(Debug)]
pub struct Guard {
    tracer: SdkTracerProvider,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer.shutdown() {
            eprintln!("{err:?}")
        }
    }
}

pub fn init_tracing_subscriber(tracing_format: TracingFormat) -> Result<Guard> {
    let provider = init_tracer_provider()?;

    // let tracer = provider.tracer(format!("{}-otel-subscriber", env!("CARGO_PKG_NAME")));

    match tracing_format {
        TracingFormat::Text => tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(
                tracing_subscriber::fmt::layer()
                    .with_level(true)
                    .with_line_number(true)
                    .with_thread_ids(false)
                    .with_span_events(FmtSpan::NONE),
            )
            // .with(OpenTelemetryLayer::new(tracer))
            .init(),

        TracingFormat::Json => tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer().json())
            // .with(OpenTelemetryLayer::new(tracer))
            .init(),
    }

    Ok(Guard { tracer: provider })
}
