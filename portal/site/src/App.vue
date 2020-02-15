<template>
    <div id="app">
      <h1>Bitcoin RS Portal</h1>
        <tabbed-node-info/>
  </div>
</template>

<script>
  import TabbedNodeInfo from "./components/TabbedNodeInfo";
  export default {
    name: 'app',
    components: {
        TabbedNodeInfo,
    }
  }
</script>

<style>
#app {
  font-family: 'Avenir', Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-align: center;
  color: #2c3e50;
  margin-top: 20px;
}
</style>
