use super::*;

pub(super) struct BootstrapTasks {
    pub(super) runtime_events: Task<()>,
}

pub(super) struct PerformanceState {
    pub(super) monitor: Option<crate::app::infrastructure::performance::PerformanceMonitor>,
    pub(super) task: Option<Task<()>>,
}

pub(super) fn spawn(runtime: &RuntimeHandle, cx: &mut Context<FarcasterApp>) -> BootstrapTasks {
    let runtime_wake = runtime.wake_receiver();
    let runtime_events = cx.spawn(async move |weak, cx| {
        while runtime_wake.recv().await.is_ok() {
            if weak.update(cx, |this, cx| this.drain_runtime(cx)).is_err() {
                break;
            }
        }
    });
    BootstrapTasks { runtime_events }
}

pub(super) fn start_performance_monitor(
    window: &Window,
    cx: &mut Context<FarcasterApp>,
) -> PerformanceState {
    let debug = std::env::var("DEBUG").ok().as_deref() == Some("true");
    let monitor = Some(
        crate::app::infrastructure::performance::PerformanceMonitor::new(
            window.window_handle().window_id(),
            debug,
        ),
    );
    let task = Some(cx.spawn(async move |weak, cx| {
        loop {
            cx.background_executor()
                .timer(crate::app::infrastructure::performance::sample_interval())
                .await;
            if weak
                .update(cx, |this, cx| {
                    if this.lifecycle.performance_monitor.as_mut().is_some_and(
                        crate::app::infrastructure::performance::PerformanceMonitor::sample_if_due,
                    ) {
                        this.notify_run_panel(cx);
                    }
                })
                .is_err()
            {
                break;
            }
        }
    }));

    PerformanceState { monitor, task }
}
