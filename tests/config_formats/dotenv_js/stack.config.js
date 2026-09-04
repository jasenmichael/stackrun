export default {
  commands: [
    {
      name: "from-dotenv-js",
      run: process.env.APP_DIR,
      cwd: process.env.APP_DIR,
    },
  ],
};
