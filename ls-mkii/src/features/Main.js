import React from 'react';
import ReactDOM from 'react-dom';
import '../components/Global.css';
import App from '../components/App';
import 'normalize.css';
import Core from './Core';


export default class Main {
    constructor() {
        this.renderRoot();
        this.reportWebVitals(console.log);

        this.core = new Core();
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