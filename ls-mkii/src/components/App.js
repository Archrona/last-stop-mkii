import './Global.css';
import './App.css';


function Header() {
    return (<div className="Header">
        <span className="HeaderLogo">⚝</span>
        <span className="HeaderLogoText">Last Stop </span>
        <span className="HeaderLogoVersion">Mk. II</span>

    </div>);
}

function LeftBodyRight() {
    return (<div className="LeftBodyRight"></div>);
}

function Footer() {
    return <div className="Footer">Asdf</div>
}

function HeaderBodyFooter() {
    let items = [];

    items.push(<Header></Header>);
    items.push(<LeftBodyRight></LeftBodyRight>);
    items.push(<Footer></Footer>);

    return (
        <div className="HeaderBodyFooter">
            {items}
        </div>
    )
}

function App() {
    return (
        <div className="App">
            <HeaderBodyFooter>

            </HeaderBodyFooter>
        </div>
    );
}

export default App;
