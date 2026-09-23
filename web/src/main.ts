import App from './App.svelte'
import './app.css'
import { mount } from 'svelte'

export default mount(App, { target: document.getElementById('app')! })
