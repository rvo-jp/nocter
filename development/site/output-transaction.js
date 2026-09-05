const fs = require("fs");
const path = require("path");

class OutputTransaction {
    constructor(projectRoot, staticRoot, finalRoot) {
        this.staticRoot = staticRoot;
        this.finalRoot = finalRoot;
        this.directory = path.join(projectRoot, `.docs-generated-${process.pid}`);
        this.previousRoot = path.join(projectRoot, `.docs-previous-${process.pid}`);
        this.state = "new";
    }

    prepare() {
        this.requireState("new");
        fs.rmSync(this.directory, { recursive: true, force: true });
        fs.mkdirSync(this.directory, { recursive: true });
        fs.cpSync(this.staticRoot, this.directory, { recursive: true });
        this.state = "prepared";
    }

    publish() {
        this.requireState("prepared");
        if (fs.existsSync(this.previousRoot)) {
            throw new Error(`Refusing to replace retained documentation output at ${this.previousRoot}`);
        }
        let previousOutputMoved = false;
        if (fs.existsSync(this.finalRoot)) {
            fs.renameSync(this.finalRoot, this.previousRoot);
            previousOutputMoved = true;
        }

        try {
            fs.renameSync(this.directory, this.finalRoot);
        } catch (error) {
            if (previousOutputMoved && !fs.existsSync(this.finalRoot)) {
                try {
                    fs.renameSync(this.previousRoot, this.finalRoot);
                } catch (restoreError) {
                    this.state = "restore-failed";
                    throw new AggregateError(
                        [error, restoreError],
                        `Documentation publication failed; previous output remains at ${this.previousRoot}`
                    );
                }
            }
            throw error;
        }

        if (previousOutputMoved) {
            fs.rmSync(this.previousRoot, { recursive: true, force: true });
        }
        this.state = "published";
    }

    cleanup() {
        fs.rmSync(this.directory, { recursive: true, force: true });
    }

    requireState(expected) {
        if (this.state !== expected) {
            throw new Error(`Documentation output transaction is ${this.state}, expected ${expected}`);
        }
    }
}

module.exports = { OutputTransaction };
