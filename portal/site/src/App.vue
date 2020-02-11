<template>
    <div id="app">
      <h1>Bitcoin RS Portal</h1>
    <info/>
  </div>
</template>

<script>
  import info from './components/Info.vue'
  export default {
    name: 'app',
    components: {'info': info}
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
