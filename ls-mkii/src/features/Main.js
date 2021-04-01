import React from 'react';
import ReactDOM from 'react-dom';
import '../components/Global.css';
import App from '../components/App';
import 'normalize.css';

const core = import('../core-pkg');

export default class Main {
    constructor() {
        this.renderRoot();
        this.reportWebVitals(console.log);

        core
            .then(m => m.greet('OH YEAH BOIIIII!'))
            .catch(console.error);
    }
    
    reportWebVitals(onPerfEntry) {
        if (onPerfEntry && onPerfEntry instanceof Function) {
            import('web-vitals').then(({ getCLS, getFID, getFCP, getLCP, getTTFB }) => {
                getCLS(onPerfEntry);
                getFID(onPerfEntry);
                getFCP(onPerfEntry);
                getLCP(onPerfEntry);
                getTTFB(onPerfEntry);
            });
        }
    };

    renderRoot() {
        ReactDOM.render(
        <React.StrictMode>
            <App />
        </React.StrictMode>,
        document.getElementById('root')
        );
    }
}