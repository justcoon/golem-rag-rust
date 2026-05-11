# Building a Durable RAG System with Golem and Rust

Most developers today are familiar with the basic Retrieval-Augmented Generation (RAG) pattern. You take some documents, chunk them, turn them into vectors, and then search through them to give an LLM some context. It sounds simple enough when you're running a script on your laptop, but things get complicated quickly when you move to production. You have to worry about what happens when an embedding service goes down halfway through a thousand-document sync, or how to handle long-running background tasks without managing complex job queues.

In this project, I decided to tackle these problems using Golem, a platform for durable execution. Instead of building a monolithic server or a collection of fragile microservices, I built a set of autonomous agents that each handle a specific part of the RAG pipeline.

### The Agentic Approach

The core idea behind Golem is that your code can be durable. When you deploy an agent, it isn't just a running process that vanishes if the server restarts. Its state is persistent, and its execution can be paused and resumed seamlessly. This changes how you think about architecture.

### The Architecture

To understand how this all fits together, it's helpful to look at the overall architecture. The system is designed to be decoupled, with each agent focusing on a single responsibility. This allows the system to remain resilient even when individual components or external services experience issues.

![Project Architecture](file:///Users/coon/workspace-zv/git/golem-rag-rust/architecture.png)

At the center of this ecosystem are several key agents:
- **SearchAgent**: Handles hybrid semantic and keyword search requests.
- **DocumentAgent**: Manages document metadata and life cycle.
- **S3DocumentSyncAgent**: Orchestrates the synchronization process and scheduling.
- **S3DocumentLoaderAgent**: Interfaces with S3 to list and fetch files.
- **EmbeddingGeneratorAgent**: Communicates with external providers to turn text into vectors.
- **DocumentEmbeddingGeneratorAgent**: Specialized worker for managing the chunking and embedding flow of individual documents.

### How Agents Collaborate

One of the most powerful features of Golem is how these agents work together. Instead of one giant service trying to do everything, the system is a small society of specialized workers.

The synchronization flow is a perfect example of this collaboration:
1. The **S3DocumentSyncAgent** acts as the orchestrator. When it's time to sync, it asks the **S3DocumentLoaderAgent** for a list of buckets.
2. For each bucket, the Sync agent triggers a parallel process. It doesn't do the work itself; instead, it delegates.
3. The **S3DocumentLoaderAgent** pulls document contents from S3 and stores them via the **DocumentAgent**.
4. Once the documents are ready, the **EmbeddingGeneratorAgent** takes over. To handle high volume, it spins up multiple **DocumentEmbeddingGeneratorAgent** instances—one for each document.

This delegation is done using **Phantom Agents**. In Golem, you can create a "phantom" instance of an agent that has its own isolated state and lifecycle. This allows us to process hundreds of documents in parallel without blocking the main sync process. If one document fails to embed because of a network glitch, Golem's durability ensures that *only* that specific agent retries, while the rest of the system moves forward.

These agents interact with a few critical external services to keep the data flowing. We use **PostgreSQL** with the pgvector extension to store both the structured metadata and the high-dimensional embeddings. The source documents themselves live in **Amazon S3**, which acts as our primary document store. Finally, we rely on an **Embedding API** (such as OpenAI) to handle the heavy mathematical lifting of vector generation.

### Out-of-the-Box Features: Endpoints, Config, and Secrets

One of the biggest hurdles in microservice development is the "glue code"—handling HTTP routing, parsing configuration files, and managing secrets. Golem handles this out of the box with a few simple annotations:

*   **HTTP Endpoints**: You don't need a separate web framework like Axum or Actix. By adding `#[endpoint]` annotations to your agent traits, Golem automatically exposes them as REST APIs. The `mount` attribute at the trait level defines the base path, making your agents reachable by any HTTP client.
*   **Typed Configuration**: Agents can receive structured configuration via `#[agent_config]`. You define a standard Rust struct with the `ConfigSchema` derive, and Golem ensures the values are correctly injected and validated at runtime.
*   **Secure Secrets**: For sensitive data like OpenAI API keys or database passwords, Golem provides a dedicated `Secret<T>` type. These are marked with `#[config_schema(secret)]`, ensuring they are handled securely, kept out of persistent logs, and never checked into source control. You manage them through the CLI or environment-specific secret stores.

One of the most interesting parts is how easy it is to define these agents in Rust. Here is a look at the trait definition for the SearchAgent:

```rust
#[derive(ConfigSchema)]
pub struct SearchAgentConfig {
    #[config_schema(nested)]
    pub embedding: EmbeddingConfig,
    #[config_schema(nested)]
    pub db: PostgresDbConfig,
}

#[agent_definition(mount = "/search", ephemeral)]
pub trait SearchAgent {
    fn new(#[agent_config] config: Config<SearchAgentConfig>) -> Self;

    #[endpoint(post = "/similar")]
    async fn find_similar_documents(
        &self,
        document_id: String,
        limit: Option<u64>,
    ) -> AgentResult<Vec<SearchResult>>;

    #[endpoint(post = "/")]
    async fn search(
        &self,
        query: String,
        filters: Option<SearchFilters>,
        limit: Option<u64>,
        threshold: Option<f32>,
        config: Option<HybridSearchConfig>,
    ) -> AgentResult<Vec<HybridSearchResult>>;
}
```

The SearchAgent is marked as ephemeral because it doesn't need to hold onto state between calls—it just performs hybrid search combining vector similarity and keyword matching. In fact, most agents in this system follow this request-response pattern, with the **S3DocumentSyncAgent** being the primary stateful component that manages its own history and scheduling.

### Handling Long-Running Tasks

The S3DocumentSyncAgent is where the durability of Golem really shines. Its job is to watch S3 buckets and ensure that every document is processed and embedded. In a traditional system, you would need a task queue like Redis or RabbitMQ to track progress and handle retries. With Golem, the agent itself tracks its history and state.

If the sync process is interrupted, Golem ensures it picks up right where it left off. I also implemented a self-scheduling mechanism so the agent can run periodically without any external cron jobs.

```rust
async fn execute_scheduled_sync(&mut self) -> AgentResult<bool> {
    log::info!("Executing scheduled sync");
    let _ = self.sync_all().await?;

    self.state.update_next_execution();

    if let Some(updated_schedule) = &self.state.sync_schedule
        && updated_schedule.is_repetitive
    {
        let schedule_time = get_next_execution_time(updated_schedule.interval_minutes);
        S3DocumentSyncAgentClient::get().schedule_execute_scheduled_sync(schedule_time);
        Ok(true)
    } else {
        self.state.delete_schedule();
        Ok(false)
    }
}
```

This snippet shows how the agent schedules its own next execution. Because the agent is durable, you don't have to worry about losing the "next run" timer if the infrastructure moves or restarts.

### Parallelism and Scale

This architecture naturally leads to a "one agent per entity" philosophy. Instead of a single worker processing a queue of documents, we have an agent for each document. 

Even though each agent execution is single-threaded to keep state management simple, the system scales by leveraging the phantom agent mechanism we discussed. By fanning out work to individual agent instances, we achieve massive parallelism while each piece of code remains dead simple—no locks, no mutexes, and no complex concurrency primitives. We just let Golem handle the distribution and durability.

Building RAG this way feels different. You aren't just writing functions that talk to a database; you're designing a small ecosystem of workers that are guaranteed to finish their jobs. It removes a huge layer of boilerplate related to reliability and state recovery, letting you focus on the actual logic of retrieval and generation.
