// Core.js

export default class Core {
    constructor() {
        this.ready = false;

        this.loadCore();
    }

    async loadCore() {
        try {
            this.core = await import('ls_core');

            console.log(this.core.dbl(10));
            console.log(this.core.dbl(36));

            this.ready = true;
            window.dbl = (x) => this.core.dbl(x);

        } catch(err) {
            console.error(`Unexpected error in loadWasm. [Message: ${err.message}]`);
        }
    }
}