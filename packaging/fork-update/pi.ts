import { spawn } from "node:child_process";
import { readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { isAbsolute } from "node:path";
import { Type } from "@earendil-works/pi-ai";
import { createBashTool, defineTool, type BashOperations, type ExtensionAPI } from "@earendil-works/pi-coding-agent";

// Only this explicit extension is loaded. Provider authentication stays in host
// Pi; no host filesystem, home directory, socket, or environment is mounted.
export default function (pi: ExtensionAPI) {
    const runtime = process.env.SPOTIFAST_UPDATE_RUNTIME ?? "";
    const container = process.env.SPOTIFAST_UPDATE_CONTAINER ?? "";
    const statePath = process.env.SPOTIFAST_UPDATE_STATE ?? "";
    const outcomePath = process.env.SPOTIFAST_UPDATE_OUTCOME ?? "";
    if (!isAbsolute(runtime) || !/^spotifast-repair-[a-f0-9]{16}$/.test(container)
        || !isAbsolute(statePath) || !isAbsolute(outcomePath)) {
        throw new Error("The updater's trusted container/session configuration is missing");
    }

    function atomic(path: string, value: unknown) {
        const temporary = `${path}.${process.pid}.tmp`;
        writeFileSync(temporary, JSON.stringify(value) + "\n", { mode: 0o600 });
        renameSync(temporary, path);
    }
    function status(phase: string, message: string) {
        const current = JSON.parse(readFileSync(statePath, "utf8"));
        atomic(statePath, { ...current, phase, message, updated_at: Math.floor(Date.now() / 1000) });
    }

    const operations: BashOperations = {
        exec: (command, _cwd, { onData, signal, timeout }) => new Promise((resolve, reject) => {
            const seconds = Math.min(Math.max(timeout ?? 120, 1), 900);
            const child = spawn(runtime, ["exec", "--workdir", "/workspace", container,
                "timeout", "--signal=TERM", "--kill-after=5s", String(seconds), "bash", "-lc", command],
                { stdio: ["ignore", "pipe", "pipe"] });
            child.stdout.on("data", onData);
            child.stderr.on("data", onData);
            let timedOut = false;
            let escalation: ReturnType<typeof setTimeout> | undefined;
            const deadline = setTimeout(() => {
                timedOut = true;
                child.kill("SIGKILL");
            }, (seconds + 15) * 1000);
            const abort = () => {
                child.kill("SIGTERM");
                escalation = setTimeout(() => child.kill("SIGKILL"), 5000);
            };
            const cleanup = () => {
                clearTimeout(deadline);
                if (escalation) clearTimeout(escalation);
                signal?.removeEventListener("abort", abort);
            };
            signal?.addEventListener("abort", abort, { once: true });
            if (signal?.aborted) abort();
            child.once("error", (error) => { cleanup(); reject(error); });
            child.once("close", (code) => {
                cleanup();
                if (timedOut) reject(new Error(`Container transport exceeded ${seconds + 15} seconds`));
                else if (signal?.aborted) reject(new Error("Container command cancelled"));
                else resolve({ exitCode: code });
            });
        }),
    };
    pi.registerTool({
        ...createBashTool("/workspace", { operations }),
        label: "Updater container",
        description: "Run a shell command inside the updater container, rooted at /workspace. Read/edit source with shell tools. No host files, credentials, desktop services or network are accessible. Do not edit tests, dependencies or updater packaging without asking the user.",
    });
    // Never fall through to local execution for terminal ! or !! commands.
    pi.on("user_bash", () => ({ operations }));

    let complete = false;
    pi.registerTool(defineTool({
        name: "fork_update_outcome",
        label: "Update repair status",
        description: "Report complete only after the source repair is ready for independent checks. Use needs_input with a plain-language question to pause for the user in this same terminal. This tool never approves installation or bypasses tests.",
        parameters: Type.Object({
            result: Type.Union([Type.Literal("complete"), Type.Literal("needs_input")]),
            message: Type.String({ minLength: 1, maxLength: 4000 }),
        }),
        async execute(_id, parameters) {
            complete = parameters.result === "complete";
            atomic(outcomePath, parameters);
            status(complete ? "checking" : "needs_input", parameters.message);
            return { content: [{ type: "text", text: complete
                ? "Repair recorded. Independent checks still decide whether it is usable."
                : "Ask this question in plain chat, then wait here for the user. No questionnaire tools are available." }],
                details: parameters, terminate: true };
        },
    }));
    pi.on("before_agent_start", () => {
        if (!complete) {
            try { unlinkSync(outcomePath); } catch (error: any) {
                if (error.code !== "ENOENT") throw error;
            }
            status("repairing", "Pi is checking the integration in the isolated container…");
        }
    });
    pi.on("agent_settled", (_event, context) => {
        if (complete) context.shutdown();
        else if (JSON.parse(readFileSync(statePath, "utf8")).phase !== "needs_input") {
            status("needs_input", "Pi is waiting in the updater terminal. Open it to see the question or continue the conversation.");
        }
    });
}
